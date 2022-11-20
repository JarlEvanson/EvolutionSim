use rand::{thread_rng, Rng};

use crate::{
    config::MutR,
    gene::{Gene, NodeID, NodeType, INNER_NODE_COUNT, INPUT_NODE_COUNT},
    grid::GridValueT,
    neuron::NeuralNet,
    steps, TimeT,
};

pub struct MovementData {
    pub x: GridValueT,
    pub y: GridValueT,
    pub lastMoveDir: DIR,
}

impl MovementData {
    pub fn getCoords(&self) -> (GridValueT, GridValueT) {
        (self.x, self.y)
    }

    pub fn setCoords(&mut self, coords: (GridValueT, GridValueT)) {
        self.x = coords.0;
        self.y = coords.1;
    }
}

pub struct NeuronData {
    neuralNet: NeuralNet,
    oscillatorPeriod: TimeT,
}

impl NeuronData {
    pub fn getOscillatorPeriod(&self) -> TimeT {
        self.oscillatorPeriod
    }
}

pub struct OtherData {
    pub color: (u8, u8, u8),
    pub isAlive: bool,
    genome: Box<[Gene]>,
}

pub fn newRandom(genomeSize: usize, stepsPerGen: TimeT) -> (MovementData, NeuronData, OtherData) {
    let mut genome = Vec::with_capacity(genomeSize);

    let mut rng = thread_rng();

    for _ in 0..genomeSize {
        genome.push(Gene::newRandom(&mut rng));
    }

    let genome = genome.into_boxed_slice();

    new(genome, thread_rng().gen::<TimeT>(), stepsPerGen)
}

pub fn new(
    genome: Box<[Gene]>,
    oscillatorPeriod: TimeT,
    stepsPerGen: TimeT,
) -> (MovementData, NeuronData, OtherData) {
    let movementData = MovementData {
        x: 0,
        y: 0,
        lastMoveDir: DIR::get_random(),
    };

    let neuronData = NeuronData {
        oscillatorPeriod: oscillatorPeriod % stepsPerGen,
        neuralNet: NeuralNet::new(&genome),
    };

    let otherData = OtherData {
        color: createColor(&genome),
        genome,
        isAlive: true,
    };

    (movementData, neuronData, otherData)
}

pub fn sexuallyReproduce(
    cell1: (&OtherData, &NeuronData),
    cell2: (&OtherData, &NeuronData),
    genomeLength: usize,
    stepsPerGen: TimeT,
    mutationRate: MutR,
) -> (MovementData, NeuronData, OtherData) {
    let mut newGenes = Vec::with_capacity(genomeLength);

    for i in 0..genomeLength {
        if thread_rng().gen_bool(0.5) {
            if thread_rng().gen_range(0.0 as f32..100.0) < mutationRate {
                let bit = thread_rng().gen_range(0..32 as u32);
                (*newGenes)[i as usize] = (*cell1.0.genome)[i as usize] ^ (1 << (bit & 31));
            }
        } else {
            if thread_rng().gen_range(0.0 as f32..100.0) < mutationRate {
                let bit = thread_rng().gen_range(0..32 as u32);
                (*newGenes)[i as usize] = (*cell2.0.genome)[i as usize] ^ (1 << (bit & 31));
            }
        }
    }

    let mut oscillator;

    if thread_rng().gen_bool(0.5) {
        oscillator = cell1.1.oscillatorPeriod;
    } else {
        oscillator = cell2.1.oscillatorPeriod;
    }

    if thread_rng().gen_range(0.0 as f32..100.0) < mutationRate {
        let bit = thread_rng().gen_range(0..32 as u32);
        oscillator = oscillator ^ (1 << (bit & 31));
    }

    let newGenes = newGenes.into_boxed_slice();

    new(newGenes, oscillator, stepsPerGen)
}

//Takes OtherData and the oscillatorPeriod
pub fn asexuallyReproduce(
    cell: (&OtherData, usize), //usize is oscillatorPeriod
    genomeLength: usize,
    stepsPerGen: TimeT,
    mutationRate: MutR,
) -> (MovementData, NeuronData, OtherData) {
    let mut newGenes = cell.0.genome.clone();

    for i in 0..genomeLength {
        if thread_rng().gen_range(0.0 as f32..100.0) < mutationRate {
            let bit = thread_rng().gen_range(0..32 as u32);
            unsafe {
                *newGenes.as_mut_ptr().add(i as usize) =
                    *newGenes.as_ptr().add(i as usize) ^ (1 << (bit & 31));
            }
        }
    }

    let mut oscillator = cell.1;

    if thread_rng().gen_range(0.0 as f32..100.0) < mutationRate {
        let bit = thread_rng().gen_range(0..32 as u32);
        oscillator = oscillator ^ (1 << (bit & 31));
    }

    new(newGenes, oscillator, stepsPerGen)
}

pub fn oneStep(
    cell: (&mut NeuronData, &MovementData),
    gridWidth: GridValueT,
    gridHeight: GridValueT,
    stepsPerGen: TimeT,
) -> (GridValueT, GridValueT) {
    unsafe {
        cell.0.neuralNet.feed_forward(&vec![
            (2 * cell.1.x) as f32 / (gridWidth as f32) - 1.0,
            (2 * cell.1.y) as f32 / (gridHeight as f32) - 1.0,
            steps as f32 / (stepsPerGen as f32),
            ((((steps as f32 / (cell.0.oscillatorPeriod as f32)) as i32 % 2) * 2) - 1) as f32,
        ]);
    }

    let outputs = cell.0.neuralNet.get_outputs();

    let offset = DIR::get_random().get_move_offset();

    let mut x = outputs[NodeID::get_index(&NodeID::MoveEast) - INPUT_NODE_COUNT - INNER_NODE_COUNT]
        - outputs[NodeID::get_index(&NodeID::MoveWest) - INPUT_NODE_COUNT - INNER_NODE_COUNT]
        + outputs[NodeID::get_index(&NodeID::MoveRandom) - INPUT_NODE_COUNT - INNER_NODE_COUNT]
            * offset.0
        + outputs[NodeID::get_index(&NodeID::MoveForward) - INPUT_NODE_COUNT - INNER_NODE_COUNT]
            * cell.1.lastMoveDir.get_move_offset().0
        + outputs[NodeID::get_index(&NodeID::MoveReverse) - INPUT_NODE_COUNT - INNER_NODE_COUNT]
            * cell.1.lastMoveDir.rotate180().get_move_offset().0
        + outputs[NodeID::get_index(&NodeID::MoveLeft) - INPUT_NODE_COUNT - INNER_NODE_COUNT]
            * cell.1.lastMoveDir.rotateCCW90().get_move_offset().0
        + outputs[NodeID::get_index(&NodeID::MoveRight) - INPUT_NODE_COUNT - INNER_NODE_COUNT]
            * cell.1.lastMoveDir.rotateCW90().get_move_offset().0;

    let mut y = outputs
        [NodeID::get_index(&NodeID::MoveNorth) - INPUT_NODE_COUNT - INNER_NODE_COUNT]
        - outputs[NodeID::get_index(&NodeID::MoveSouth) - INPUT_NODE_COUNT - INNER_NODE_COUNT]
        + outputs[NodeID::get_index(&NodeID::MoveRandom) - INPUT_NODE_COUNT - INNER_NODE_COUNT]
            * offset.1
        + outputs[NodeID::get_index(&NodeID::MoveForward) - INPUT_NODE_COUNT - INNER_NODE_COUNT]
            * cell.1.lastMoveDir.get_move_offset().1
        + outputs[NodeID::get_index(&NodeID::MoveReverse) - INPUT_NODE_COUNT - INNER_NODE_COUNT]
            * cell.1.lastMoveDir.get_move_offset().1
        + outputs[NodeID::get_index(&NodeID::MoveLeft) - INPUT_NODE_COUNT - INNER_NODE_COUNT]
            * cell.1.lastMoveDir.rotateCCW90().get_move_offset().1
        + outputs[NodeID::get_index(&NodeID::MoveRight) - INPUT_NODE_COUNT - INNER_NODE_COUNT]
            * cell.1.lastMoveDir.rotateCW90().get_move_offset().1;

    x = x.tanh();
    y = y.tanh();

    let mut coords = (cell.1.x, cell.1.y);

    if (thread_rng().gen_range(0..i32::MAX) as f32) / (i32::MAX as f32) < x.abs() {
        if x > 0.0 {
            coords.0 = coords.0 + 1;
        } else {
            coords.0 = coords.0.saturating_sub(1);
        }
    }

    if coords.0 >= gridWidth {
        coords.0 = gridWidth - 1;
    }

    if (thread_rng().gen_range(0..i32::MAX) as f32) / (i32::MAX as f32) < y.abs() {
        if y > 0.0 {
            coords.1 = coords.1 + 1;
        } else {
            coords.1 = coords.1.saturating_sub(1);
        }
    }

    if coords.1 >= gridHeight {
        coords.1 = gridHeight - 1;
    }

    coords
}

pub fn createColor(genome: &Box<[Gene]>) -> (u8, u8, u8) {
    const maxColorVal: u32 = 0xb0;
    const maxLumaVal: u32 = 0xb0;

    let mut color = {
        let c: u32 = u32::from(genome.first().unwrap().get_head_type() == NodeType::INPUT)
            | (u32::from(genome.last().unwrap().get_head_type() == NodeType::INPUT) << 1)
            | (u32::from(genome.first().unwrap().get_tail_type() == NodeType::INNER) << 2)
            | (u32::from(genome.last().unwrap().get_tail_type() == NodeType::INNER) << 3)
            | (((genome.first().unwrap().get_head_node_id().get_index() & 1) as u32) << 4)
            | (((genome.first().unwrap().get_tail_node_id().get_index() & 1) as u32) << 5)
            | (((genome.last().unwrap().get_head_node_id().get_index() & 1) as u32) << 6)
            | (((genome.last().unwrap().get_tail_node_id().get_index() & 1) as u32) << 7);

        (c, ((c & 0x1f) << 3), ((c & 7) << 5))
    };

    if (color.0 * 3 + color.1 + color.2 * 4) / 8 > maxLumaVal {
        if color.0 > maxColorVal {
            color.0 %= maxColorVal
        };
        if color.1 > maxColorVal {
            color.1 %= maxColorVal
        };
        if color.2 > maxColorVal {
            color.2 %= maxColorVal
        };
    }

    (color.0 as u8, color.1 as u8, color.2 as u8)
}

#[derive(Debug)]
pub enum DIR {
    North,
    NorthEast,
    East,
    SouthEast,
    South,
    SouthWest,
    West,
    NorthWest,
}

impl DIR {
    pub fn get_move_offset(&self) -> (f32, f32) {
        match *self {
            DIR::North => (0.0, 1.0),
            DIR::NorthEast => (1.0, 1.0),
            DIR::East => (1.0, 0.0),
            DIR::SouthEast => (1.0, -1.0),
            DIR::South => (0.0, -1.0),
            DIR::SouthWest => (-1.0, -1.0),
            DIR::West => (-1.0, 0.0),
            DIR::NorthWest => (-1.0, 1.0),
        }
    }

    pub fn get_random() -> DIR {
        match rand::thread_rng().gen_range(0..8) {
            0 => DIR::North,
            1 => DIR::NorthEast,
            2 => DIR::East,
            3 => DIR::SouthEast,
            4 => DIR::South,
            5 => DIR::SouthWest,
            6 => DIR::West,
            7 => DIR::NorthWest,
            _ => unreachable!(),
        }
    }

    pub fn get_dir_from_offset(offset: (isize, isize)) -> DIR {
        match offset {
            (0, 1) => DIR::North,
            (1, 1) => DIR::NorthEast,
            (1, 0) => DIR::East,
            (1, -1) => DIR::SouthEast,
            (0, -1) => DIR::South,
            (-1, -1) => DIR::SouthWest,
            (-1, 0) => DIR::West,
            (-1, 1) => DIR::NorthWest,
            (_, _) => unimplemented!(),
        }
    }

    pub fn rotateCCW90(&self) -> DIR {
        match *self {
            DIR::North => DIR::West,
            DIR::NorthEast => DIR::NorthWest,
            DIR::East => DIR::North,
            DIR::SouthEast => DIR::NorthEast,
            DIR::South => DIR::East,
            DIR::SouthWest => DIR::SouthEast,
            DIR::West => DIR::South,
            DIR::NorthWest => DIR::SouthWest,
        }
    }

    pub fn rotateCW90(&self) -> DIR {
        match *self {
            DIR::West => DIR::North,
            DIR::NorthWest => DIR::NorthEast,
            DIR::North => DIR::East,
            DIR::NorthEast => DIR::SouthEast,
            DIR::East => DIR::South,
            DIR::SouthEast => DIR::SouthWest,
            DIR::South => DIR::West,
            DIR::SouthWest => DIR::NorthWest,
        }
    }

    pub fn rotate180(&self) -> DIR {
        match *self {
            DIR::North => DIR::South,
            DIR::NorthEast => DIR::SouthWest,
            DIR::East => DIR::West,
            DIR::SouthEast => DIR::NorthWest,
            DIR::South => DIR::North,
            DIR::SouthWest => DIR::NorthEast,
            DIR::West => DIR::East,
            DIR::NorthWest => DIR::SouthEast,
        }
    }
}
