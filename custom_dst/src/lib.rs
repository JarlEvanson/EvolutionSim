//! Custom Dynamically sized types.
#![feature(ptr_metadata)]

use std::{
    alloc::Layout,
    marker::PhantomData,
    mem::MaybeUninit,
    ops::{Index, IndexMut},
    ptr::{self, NonNull},
};

pub struct MaybeUninitDstArray<Header, Tail> {
    ptr: NonNull<Header>,
    entry_count: usize,
    tail_count: usize,
    phantom_data: PhantomData<Tail>,
}

impl<Header, Tail> MaybeUninitDstArray<Header, Tail> {
    pub fn new(tail_count: usize, entry_count: usize) -> Self {
        let (total_layout, _) = entry_layout::<Header, Tail>(tail_count)
            .repeat(entry_count)
            .unwrap();
        // SAFETY:
        //
        // TODO:
        let ptr = unsafe { std::alloc::alloc(total_layout) };
        let Some(ptr) = NonNull::new(ptr) else {
            std::alloc::handle_alloc_error(total_layout);
        };

        Self {
            ptr: ptr.cast(),
            entry_count,
            tail_count,
            phantom_data: PhantomData,
        }
    }

    pub fn assume_init(self) -> DstArray<Header, Tail> {
        DstArray {
            ptr: self.ptr,
            entry_count: self.entry_count,
            tail_count: self.tail_count,
            phantom_data: self.phantom_data,
        }
    }

    pub fn write_header(&mut self, index: usize, header: Header) {
        self.create_maybe_uninit_mut().get_arr_element_mut(index).get_header_mut().write(header);
    }

    pub fn get_footer_ptr(&self, index: usize) -> *mut [Tail] {
        (&raw const self.create().get_arr_element(index).tail).cast_mut()
    }

    pub fn get_footer_element_ptr_mut(&mut self, index: usize, subindex: usize) -> *mut Tail {
        (&raw mut self.create_mut().get_arr_element_mut(index).tail[subindex])
    }

    fn create_maybe_uninit_mut(&mut self) -> DstArray<MaybeUninit<Header>, MaybeUninit<Tail>> {
        DstArray {
            ptr: self.ptr.cast(),
            entry_count: self.entry_count,
            tail_count: self.tail_count,
            phantom_data: PhantomData,
        }
    }

    fn create(&self) -> DstArray<Header, Tail> {
        DstArray {
            ptr: self.ptr,
            entry_count: self.entry_count,
            tail_count: self.tail_count,
            phantom_data: PhantomData,
        }
    }

    fn create_mut(&mut self) -> DstArray<Header, Tail> {
        DstArray {
            ptr: self.ptr,
            entry_count: self.entry_count,
            tail_count: self.tail_count,
            phantom_data: PhantomData,
        }
    }
}

pub struct DstArray<Header, Tail> {
    ptr: NonNull<Header>,
    entry_count: usize,
    tail_count: usize,
    phantom_data: PhantomData<Tail>,
}

impl<Header, Tail> DstArray<Header, Tail> {
    pub fn swap(&mut self, other: &mut Self) {
        assert_eq!(self.entry_count, other.entry_count);
        assert_eq!(self.tail_count, other.tail_count);
        std::mem::swap(&mut self.ptr, &mut other.ptr);
    }

    pub fn get_arr_element(&self, index: usize) -> &DstData<Header, Tail> {
        assert!(index < self.entry_count);

        let offset = index * self.entry_size();
        let ptr = self
            .ptr
            .as_ptr()
            .wrapping_byte_add(offset)
            .cast::<DstDataFixed<0, Header, Tail>>();
        // SAFETY:
        //
        // TODO
        unsafe { &*std::ptr::from_raw_parts(ptr, self.tail_count) }
    }

    pub fn get_arr_element_mut(&mut self, index: usize) -> &mut DstData<Header, Tail> {
        assert!(index < self.entry_count);

        let offset = index * self.entry_size();
        let ptr = self
            .ptr
            .as_ptr()
            .wrapping_byte_add(offset)
            .cast::<DstDataFixed<0, Header, Tail>>();
        // SAFETY:
        //
        // TODO
        unsafe { &mut *std::ptr::from_raw_parts_mut(ptr, self.tail_count) }
    }

    pub fn get_slice(&self, start: usize, length: usize) -> DstSlice<'_, Header, Tail> {
        assert!(start.strict_add(length) <= self.entry_count);
        DstSlice {
            store: self,
            start,
            length,
        }
    }

    pub fn get_mut_slice(&mut self, start: usize, length: usize) -> DstSliceMut<'_, Header, Tail> {
        assert!(start.strict_add(length) <= self.entry_count);
        DstSliceMut {
            store: self,
            start,
            length,
        }
    }

    fn entry_size(&self) -> usize {
        entry_layout::<Header, Tail>(self.tail_count).size()
    }
}

impl<Header, Tail> Index<usize> for DstArray<Header, Tail> {
    type Output = DstData<Header, Tail>;

    fn index(&self, index: usize) -> &Self::Output {
        self.get_arr_element(index)
    }
}

impl<Header, Tail> IndexMut<usize> for DstArray<Header, Tail> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        self.get_arr_element_mut(index)
    }
}

unsafe impl<Header, Tail> Send for DstArray<Header, Tail> {}
unsafe impl<Header, Tail> Sync for DstArray<Header, Tail> {}

pub struct DstSliceMut<'a, Header, Tail> {
    store: &'a mut DstArray<Header, Tail>,
    start: usize,
    length: usize,
}

impl<'a, Header, Tail> DstSliceMut<'a, Header, Tail> {
    pub fn as_shared_slice(&self) -> DstSlice<'_, Header, Tail> {
        DstSlice {
            store: self.store,
            start: self.start,
            length: self.length,
        }
    }
}

impl<Header, Tail> Index<usize> for DstSliceMut<'_, Header, Tail> {
    type Output = DstData<Header, Tail>;

    fn index(&self, index: usize) -> &Self::Output {
        assert!(index < self.length);
        self.store.get_arr_element(self.start + index)
    }
}

impl<Header, Tail> IndexMut<usize> for DstSliceMut<'_, Header, Tail> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        assert!(index < self.length);
        self.store.get_arr_element_mut(self.start + index)
    }
}

pub struct DstSlice<'a, Header, Tail> {
    store: &'a DstArray<Header, Tail>,
    start: usize,
    length: usize,
}

impl<Header, Tail> Index<usize> for DstSlice<'_, Header, Tail> {
    type Output = DstData<Header, Tail>;

    fn index(&self, index: usize) -> &Self::Output {
        assert!(index < self.length);
        self.store.get_arr_element(self.start + index)
    }
}

pub struct DstData<Header, Tail> {
    header: Header,
    tail: [Tail],
}

impl<Header, Tail> DstData<Header, Tail> {
    pub fn get_header(&self) -> &Header {
        &self.header
    }

    pub fn get_header_mut(&mut self) -> &mut Header {
        &mut self.header
    }

    pub fn get_footer(&self) -> &[Tail] {
        &self.tail
    }

    pub fn get_mut_footer(&mut self) -> &mut [Tail] {
        &mut self.tail
    }
}

struct DstDataFixed<const N: usize, Header, Tail> {
    header: Header,
    tail: [Tail; N],
}

pub fn entry_layout<Header, Tail>(tail_count: usize) -> Layout {
    let header_layout = Layout::new::<Header>();
    let tail_layout = Layout::array::<Tail>(tail_count).unwrap();
    header_layout.extend(tail_layout).unwrap().0.pad_to_align()
}
