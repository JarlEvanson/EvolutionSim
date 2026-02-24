#![allow(non_snake_case, non_upper_case_globals, temporary_cstring_as_ptr)]
#![feature(trace_macros)]
#![feature(test)]

use std::slice::{Chunks, ChunksMut};
use std::{process::exit, rc::Rc};

extern crate rand;

extern crate custom_dst;

extern crate scoped_threadpool;
use cell::NeuronData;
use custom_dst::MaybeUninitDstArray;
use rand::thread_rng;
use scoped_threadpool::Pool;

mod windowed;
use windowed::window::Window;

mod grid;
use grid::{Grid, GridValueT};

mod population;
use population::Population;

mod cell;

mod gene;
use gene::TOTAL_NODE_COUNT;

use crate::cell::HeritableData;
use crate::gene::Gene;
use crate::windowed::window::wait;
use crate::windowed::WindowingStatus;
mod neuron;

mod bench;

use DebugCell::DebugRefCell;

//Statistics
static mut neuron_presence: [u32; TOTAL_NODE_COUNT] = [0; TOTAL_NODE_COUNT];

mod config;
use config::Config as ConfigBase;

type Config = Rc<ConfigBase>;
type TimeT = usize;

fn main() {
    println!("The argument file=\"path\" will load the save");

    let config: Config = Rc::new(ConfigBase::initFromArgs());

    println!("{}", config);

    let grid = Rc::new(DebugRefCell::new(Grid::new(
        config.get_grid_width(),
        config.get_grid_height(),
    )));

    let population = Rc::new(DebugRefCell::new(Population::new(
        &config,
        &mut grid.borrowMut(),
    )));

    let scratch = MaybeUninitDstArray::<HeritableData, Gene>::new(
        config.get_genome_size(),
        config.get_pop_size(),
    );

    let mut threadpool = Pool::new(std::thread::available_parallelism().unwrap().get() as u32);

    //Safety: this will be written to before it is ever read from
    let mut scratch = { unsafe { scratch.assume_init() } };

    let mut generation = 0;

    if config.get_is_windowing() {
        println!("Press R to reset simulation\nPress SPACE to pause and restart simulation\nPress E to print current neuron frequencies\nPress Escape to close window\nPress S to save current generation's genes\nPress C to print config");

        let windowing_status = Rc::new(DebugRefCell::new(WindowingStatus {
            is_paused: false,
            should_reset: false,
        }));

        let window = Window::createWindow(&config, &windowing_status, 512, 512)
            .expect("Window failed to be created");
        window.make_current();

        window.render(&config, &population.borrow());

        let mut accounted_time = unsafe { glfw::ffi::glfwGetTime() };

        let mut outputted = false;

        let mut step = 0;

        while !window.shouldClose() {
            window.poll();
            if windowing_status.borrow().should_reset {
                step = 0;
                generation = 0;
                unsafe {
                    accounted_time = glfw::ffi::glfwGetTime();
                    windowing_status.borrowMut().should_reset = false;
                }

                population
                    .borrowMut()
                    .genRandom(&config, &mut grid.borrowMut());
            }

            if step == 0 && !outputted {
                window.render(&config, &population.borrow());

                println!("Generation {}:", generation);

                outputted = true;
            }

            if unsafe {
                glfw::ffi::glfwGetTime() - accounted_time > 0.016
                    && !windowing_status.borrow().is_paused
            } {
                outputted = false;

                accounted_time += 0.016;

                step += 1;

                let size =
                    computeMovements(&config, &mut threadpool, &mut population.borrowMut(), step);
                population
                    .borrowMut()
                    .resolveMoveQueue(size, &mut grid.borrowMut());

                determine_deaths(&config, step, &mut population.borrowMut());
                population.borrowMut().resolveDead(&mut grid.borrowMut());

                if population.borrow().getLivingIndices().is_empty() {
                    println!("Everyone Died");
                    loop {
                        window.poll();
                        if window.shouldClose() || windowing_status.borrow().should_reset {
                            unsafe { accounted_time = glfw::ffi::glfwGetTime() };
                            break;
                        }
                    }
                    continue;
                }

                window.render(&config, &population.borrow());
            }

            if step == config.get_steps_per_gen() {
                let reproducers = determine_reproducers(&config, &population.borrowMut());
                if reproducers.is_empty() {
                    println!("Failed to produce viable offspring");
                    loop {
                        window.poll();
                        if window.shouldClose() || windowing_status.borrow().should_reset {
                            unsafe { accounted_time = glfw::ffi::glfwGetTime() };
                            break;
                        }
                    }
                    continue;
                }

                println!(
                    "Dead: {:3}\tReproducing: {:3}\tLiving Non-reproducing: {:3}",
                    config.get_pop_size() - population.borrow().getLivingIndices().len(),
                    reproducers.len(),
                    population.borrow().getLivingIndices().len() - reproducers.len(),
                );

                grid.borrowMut().reset();

                population.borrowMut().reproduceAsexually(
                    &mut scratch,
                    &config,
                    reproducers,
                    &mut grid.borrowMut(),
                );

                wait(&window, &windowing_status, &mut accounted_time, 1.0);

                window.render(&config, &population.borrow());

                step = 0;
                generation += 1;
            }
        }
    } else {
        let steps_per_gen = config.get_steps_per_gen();

        #[allow(unused_labels)]
        'gen_loop: loop {
            println!("Generation {}", generation);

            for step in 0..steps_per_gen {
                let size =
                    computeMovements(&config, &mut threadpool, &mut population.borrowMut(), step);
                population
                    .borrowMut()
                    .resolveMoveQueue(size, &mut grid.borrowMut());

                determine_deaths(&config, step, &mut population.borrowMut());
                population.borrowMut().resolveDead(&mut grid.borrowMut());

                if population.borrow().getLivingIndices().is_empty() {
                    println!("Everyone Died");
                    return;
                }
            }

            let reproducers = determine_reproducers(&config, &population.borrowMut());
            if reproducers.is_empty() {
                println!("Failed to produce viable offspring");
                exit(1);
            }

            println!(
                "Dead: {:3}\tReproducing: {:3}\tLiving Non-reproducing: {:3}",
                config.get_pop_size() - population.borrow().getLivingIndices().len(),
                reproducers.len(),
                population.borrow().getLivingIndices().len() - reproducers.len(),
            );

            grid.borrowMut().reset();

            population.borrowMut().reproduceAsexually(
                &mut scratch,
                &config,
                reproducers,
                &mut grid.borrowMut(),
            );
            generation += 1;
        }
    }
}

pub fn computeMovements(
    config: &Config,
    threadpool: &mut Pool,
    pop: &mut Population,
    step: TimeT,
) -> usize {
    let living = pop.getLivingIndices();
    let len = living.len();

    let parts = {
        let threads = threadpool.thread_count();
        let (num, rem) = (
            living.len() / ((threads + 1) as usize),
            living.len() % ((threads + 1) as usize),
        );

        if rem != 0 {
            num + 1
        } else {
            num
        }
    };

    let (movement, neuron, heritable, _, results) = pop.get_data_mut();

    //General read-only data
    let (movement, heritable) = { (&*movement, &heritable.as_shared_slice()) };

    let mut neuron = Some(neuron);

    //Thread IO is chunked for lock-free reading and writing
    let mut living: Chunks<usize> = living.as_slice().chunks(parts);
    let mut resChunks: ChunksMut<(usize, (GridValueT, GridValueT))> = results.chunks_mut(parts);

    let gridWidth = config.get_grid_width();
    let gridHeight = config.get_grid_height();
    let stepsPerGen = config.get_steps_per_gen();

    threadpool.scoped(|scope| {
        //Chunked IO for main thread
        let local_living = living.next().unwrap();
        let localResults = resChunks.next().unwrap();

        let mut last_included_cell = local_living.last().unwrap();
        let mut start_index;

        //# Safety
        //
        // neuron is declared as a some above, so it must be a some
        let local_neuron = unsafe {
            let (first, last) = split_or_get(neuron.unwrap_unchecked(), last_included_cell + 1);
            //We add two to get the correct index because the first slice doesn't include the index
            neuron = last;
            first
        };

        for living_chunk in living {
            start_index = last_included_cell + 1;
            last_included_cell = living_chunk.last().unwrap();

            let neuron_chunk = {
                //# Safety
                //
                //The splitting values will only be equal to the length of the chunk at the end,
                //and so it will always be Some
                let (fst, lst) = unsafe {
                    split_or_get(
                        neuron.unwrap_unchecked(),
                        last_included_cell - start_index + 1,
                    )
                };
                neuron = lst;
                fst
            };
            let resChunk = resChunks.next().unwrap();

            scope.execute(move || {
                let mut rng = thread_rng();

                for (index, cellIndex) in living_chunk.iter().enumerate() {
                    let movement = &movement[*cellIndex];
                    let heritable_data = &heritable[*cellIndex];

                    let neurons = &mut neuron_chunk[(*cellIndex) - start_index];

                    let coords = cell::one_step(
                        neurons,
                        movement,
                        heritable_data.get_header().get_oscillator(),
                        step,
                        gridWidth,
                        gridHeight,
                        stepsPerGen,
                        &mut rng,
                    );
                    resChunk[index] = (*cellIndex, coords);
                }
            });
        }

        let mut rng = thread_rng();

        for (index, cellIndex) in local_living.iter().enumerate() {
            let movement = &movement[*cellIndex];
            let heritable_data = &heritable[*cellIndex];

            let neurons = &mut local_neuron[*cellIndex];

            let coords = cell::one_step(
                neurons,
                movement,
                heritable_data.get_header().get_oscillator(),
                step,
                gridWidth,
                gridHeight,
                stepsPerGen,
                &mut rng,
            );
            localResults[index] = (*cellIndex, coords);
        }
    });

    len
}

fn split_or_get(
    data: &mut [NeuronData],
    index: usize,
) -> (&mut [NeuronData], Option<&mut [NeuronData]>) {
    if index == data.len() {
        (data, None)
    } else {
        let (fst, lst) = data.split_at_mut(index);
        (fst, Some(lst))
    }
}
pub fn determine_reproducers(config: &Config, pop: &Population) -> Vec<usize> {
    let mut reproducers = Vec::new();
    for cellIndex in pop.getLivingIndices() {
        let cell = pop.getCellMovementData(cellIndex);
        if cell.getCoords().0 < config.get_grid_width() / 4
            || cell.getCoords().0 > 3 * config.get_grid_width() / 4
        {
            reproducers.push(cellIndex);
        }
    }

    reproducers
}

pub fn determine_deaths(config: &Config, step: TimeT, pop: &mut Population) {
    if step == config.get_steps_per_gen() / 4 {
        for index in &pop.getLivingIndices() {
            let (x, _) = pop.getCellMovementData(*index).getCoords();

            if x < config.get_grid_width() / 4 || x > (3 * config.get_grid_width()) / 4 {
                pop.addToDeathQueue(*index)
            }
        }
    } else if step == config.get_steps_per_gen() / 2 {
        for index in &pop.getLivingIndices() {
            let (x, _) = pop.getCellMovementData(*index).getCoords();

            if x > config.get_grid_width() / 4 && x < (3 * config.get_grid_width()) / 4 {
                pop.addToDeathQueue(*index)
            }
        }
    } else if step == (3 * config.get_steps_per_gen()) / 4 {
        for index in &pop.getLivingIndices() {
            let (x, _) = pop.getCellMovementData(*index).getCoords();

            if x < config.get_grid_width() / 4 || x > (3 * config.get_grid_width()) / 4 {
                pop.addToDeathQueue(*index)
            }
        }
    }

    if pop.getDeathQueueLen() > 0 {
        println!("Step {} Killed: {}", step, pop.getDeathQueueLen());
    }
}

pub fn save(config: &Config, generation: TimeT, population: &Population) {}

mod DebugCell {
    use std::{
        cell::UnsafeCell,
        ops::{Deref, DerefMut},
    };

    #[cfg(debug_assertions)]
    #[derive(Clone, Copy, Debug, PartialEq)]
    enum RefType {
        Mutable,
        Immutable(usize),
        None,
    }

    pub struct Ref<'a, T> {
        reference: &'a T,
        #[cfg(debug_assertions)]
        counter: &'a mut RefType,
    }

    impl<'a, T> Deref for Ref<'a, T> {
        type Target = T;

        fn deref(&self) -> &Self::Target {
            self.reference
        }
    }

    impl<'a, T> Drop for Ref<'a, T> {
        fn drop(&mut self) {
            #[cfg(debug_assertions)]
            if let RefType::Immutable(count) = self.counter {
                if *count == 0 {
                    panic!("Leaked memory!, invalid");
                } else if *count == 1 {
                    *self.counter = RefType::None;
                    return;
                } else {
                    *count -= 1;
                    return;
                }
            }
            #[cfg(debug_assertions)]
            panic!("Invalid type");
        }
    }

    pub struct RefMut<'a, T> {
        reference: &'a mut T,
        #[cfg(debug_assertions)]
        counter: &'a mut RefType,
    }

    impl<'a, T> Deref for RefMut<'a, T> {
        type Target = T;

        fn deref(&self) -> &Self::Target {
            self.reference
        }
    }

    impl<'a, T> DerefMut for RefMut<'a, T> {
        fn deref_mut(&mut self) -> &mut Self::Target {
            self.reference
        }
    }

    impl<'a, T> Drop for RefMut<'a, T> {
        fn drop(&mut self) {
            #[cfg(debug_assertions)]
            if let RefType::Mutable = self.counter {
                *self.counter = RefType::None;
                return;
            }
            #[cfg(debug_assertions)]
            panic!("Invalid type");
        }
    }

    pub struct DebugRefCell<T> {
        interior: UnsafeCell<T>,
        #[cfg(debug_assertions)]
        refType: UnsafeCell<RefType>,
    }

    impl<T> DebugRefCell<T> {
        pub fn new(value: T) -> DebugRefCell<T> {
            #[cfg(debug_assertions)]
            return DebugRefCell {
                interior: UnsafeCell::new(value),
                refType: UnsafeCell::new(RefType::None),
            };
            #[cfg(not(debug_assertions))]
            return DebugRefCell {
                interior: UnsafeCell::new(value),
            };
        }

        pub fn borrow<'a, 'b>(&'b self) -> Ref<'a, T>
        where
            'b: 'a,
        {
            #[cfg(debug_assertions)]
            let kind = unsafe { *self.refType.get() };
            #[cfg(debug_assertions)]
            match kind {
                RefType::None => unsafe {
                    *self.refType.get() = RefType::Immutable(1);
                    let reference = &*self.interior.get().cast::<T>();
                    Ref {
                        reference,
                        counter: &mut *self.refType.get(),
                    }
                },
                RefType::Immutable(count) => unsafe {
                    *self.refType.get() = RefType::Immutable(count.checked_add(1).unwrap());
                    let reference = &*self.interior.get().cast::<T>();
                    Ref {
                        reference,
                        counter: &mut *self.refType.get(),
                    }
                },
                RefType::Mutable => {
                    panic!("Cannot borrow a immutable reference and a mutable reference at the same time");
                }
            }
            #[cfg(not(debug_assertions))]
            unsafe {
                Ref {
                    reference: &*self.interior.get().cast::<T>(),
                }
            }
        }

        pub fn borrowMut<'a, 'b>(&'b self) -> RefMut<'a, T>
        where
            'b: 'a,
        {
            #[cfg(debug_assertions)]
            let kind = unsafe { *self.refType.get() };
            #[cfg(debug_assertions)]
            match kind {
                RefType::None => unsafe {
                    *self.refType.get() = RefType::Mutable;
                    let reference = &mut *self.interior.get().cast::<T>();
                    RefMut {
                        reference,
                        counter: &mut *self.refType.get(),
                    }
                },
                RefType::Immutable(_) => panic!(
                    "Cannot borrow a immutable reference and a mutable reference at the same time"
                ),
                RefType::Mutable => panic!("Cannot borrow multiple mutable references"),
            }
            #[cfg(not(debug_assertions))]
            unsafe {
                RefMut {
                    reference: &mut *self.interior.get().cast::<T>(),
                }
            }
        }

        #[cfg(debug_assertions)]
        #[allow(unused)]
        fn getRefType(&self) -> RefType {
            unsafe { *self.refType.get() }
        }
    }

    #[test]
    #[cfg(debug_assertions)]
    fn refDropTest() {
        let cell = DebugRefCell::new(0 as usize);

        let x = cell.borrow();
        let y = cell.borrow();
        let z = cell.borrow();

        assert_eq!(cell.getRefType(), RefType::Immutable(3));

        std::mem::drop(x);

        assert_eq!(cell.getRefType(), RefType::Immutable(2));

        std::mem::drop(z);

        assert_eq!(cell.getRefType(), RefType::Immutable(1));

        std::mem::drop(y);

        assert_eq!(cell.getRefType(), RefType::None);
    }

    #[test]
    #[cfg(debug_assertions)]
    fn refMutDropTest() {
        let cell = DebugRefCell::new(0 as usize);

        let x = cell.borrowMut();

        assert_eq!(cell.getRefType(), RefType::Mutable);

        std::mem::drop(x);
    }

    #[test]
    #[should_panic]
    #[cfg(debug_assertions)]
    fn refImmAliasTest() {
        let cell = DebugRefCell::new(0 as usize);

        let _x = cell.borrow();
        let _y = cell.borrow();

        cell.borrowMut();
    }

    #[test]
    #[should_panic]
    #[cfg(debug_assertions)]
    fn refMutAliasTest() {
        let cell = DebugRefCell::new(0 as usize);

        let _z = cell.borrowMut();

        let _x = cell.borrow();
        let _y = cell.borrow();
    }

    #[test]
    #[should_panic]
    #[cfg(debug_assertions)]
    fn refMultiMut() {
        let cell = DebugRefCell::new(0 as usize);

        let _z = cell.borrowMut();

        let _x = cell.borrowMut();
    }
}
