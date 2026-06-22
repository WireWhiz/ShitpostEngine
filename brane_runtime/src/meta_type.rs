use std::{cell::UnsafeCell, collections::HashMap, ops::Deref, sync::OnceLock};

use crate::{define_stable_key, stable_table::StableTable};

/// Globally unique ID for a meta_type.
///
/// Must be constructed with global_unique_usize!() to ensure no collisions after resolve
///
/// On compile and hot-reload these IDs are not guarenteed to be consistent, so MetaTypeDefinition::ident must
/// be used for hot-reload diffing instead.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct MetaTypeId(pub usize);

#[macro_export]
macro_rules! global_unique_usize {
    () => {{
        static ANCHOR: () = ();
        &ANCHOR as *const () as usize
    }};
}

#[derive(Debug)]
pub struct MetaTypeDefinition {
    pub id: MetaTypeId,
    pub ident: &'static str,
    pub byte_size: usize,
    pub fields: &'static [MetaFieldDefinition],
    // Drop a value of this type at the specified address
    pub drop: unsafe fn(*mut u8),
}

#[derive(Debug)]
pub struct MetaFieldDefinition {
    pub ident: &'static str,
    pub byte_offset: usize,
    pub def: &'static MetaTypeDefinition,
}

impl MetaTypeDefinition {
    pub unsafe fn from_slice<T: MetaType>(&self, data: &[u8]) -> &T {
        assert_eq!(data.len(), self.byte_size);
        unsafe { &*(data.as_ptr() as *const T) }
    }

    pub unsafe fn from_slice_mut<T: MetaType>(&self, data: &mut [u8]) -> &mut T {
        assert_eq!(data.len(), self.byte_size);
        unsafe { &mut *(data.as_ptr() as *mut T) }
    }

    pub fn get_field_data<'a>(&self, data: &'a [u8], field_index: usize) -> &'a [u8] {
        assert_eq!(data.len(), self.byte_size);
        assert!(field_index < self.fields.len());

        let field = &self.fields[field_index];

        let start = field.byte_offset;
        let end = start + field.def.byte_size;
        &data[start..end]
    }

    pub unsafe fn get_field<'a, T: MetaType>(&self, data: &[u8], field_index: usize) -> &T {
        let field = &self.fields[field_index];
        assert_eq!(field.def.id, T::meta_id());
        unsafe { &*(self.get_field_data(data, field_index).as_ptr() as *const T) }
    }

    pub fn get_field_data_mut<'a>(&self, data: &'a mut [u8], field_index: usize) -> &'a mut [u8] {
        assert_eq!(data.len(), self.byte_size);
        assert!(field_index < self.fields.len());

        let field = &self.fields[field_index];

        let start = field.byte_offset;
        let end = start + field.def.byte_size;
        &mut data[start..end]
    }

    pub unsafe fn get_field_mut<'a, T: MetaType>(&self, data: &mut [u8], field_index: usize) -> &T {
        let field = &self.fields[field_index];
        assert_eq!(field.def.id, T::meta_id());
        unsafe { &mut *(self.get_field_data_mut(data, field_index).as_ptr() as *mut T) }
    }
}

pub trait MetaType: Sized + Unpin {
    fn meta_id() -> MetaTypeId;
    fn meta_def() -> &'static MetaTypeDefinition;
    fn as_bytes(&self) -> &[u8] {
        unsafe {
            std::slice::from_raw_parts(
                self as *const Self as *const u8,
                std::mem::size_of::<Self>(),
            )
        }
    }
}

#[derive(Default)]
pub struct MetaTypeLibrary {
    pub id_to_def: HashMap<MetaTypeId, &'static MetaTypeDefinition>,
    pub ident_to_def: HashMap<&'static str, &'static MetaTypeDefinition>,
}

impl MetaTypeLibrary {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_type<T: MetaType>(&mut self) {
        self.register_type_by_def(T::meta_def());
    }

    pub fn register_type_by_def(&mut self, def: &'static MetaTypeDefinition) {
        let had_id = self.id_to_def.insert(def.id, def).is_some();
        assert_eq!(had_id, false);
        let had_ident = self.ident_to_def.insert(def.ident, def).is_some();
        assert_eq!(had_ident, false);
    }

    // TODO dealocation stuff
}

/// Stores structs defined by meta-types at runtime.
pub struct MetaValueVec {
    data: Vec<u8>,
    def: &'static MetaTypeDefinition,
}

impl MetaValueVec {
    pub fn new(def: &'static MetaTypeDefinition) -> Self {
        Self {
            data: Vec::new(),
            def,
        }
    }

    pub fn get_def(&self) -> &'static MetaTypeDefinition {
        self.def
    }

    pub fn get_index_range(&self, index: usize) -> std::ops::Range<usize> {
        let stride = self.def.byte_size;
        let start = index * stride;
        let end = start + stride;
        start..end
    }

    pub fn get(&self, index: usize) -> &[u8] {
        &self.data[self.get_index_range(index)]
    }

    pub fn get_mut(&mut self, index: usize) -> &mut [u8] {
        let range = self.get_index_range(index);
        &mut self.data[range]
    }

    pub fn push(&mut self, data: &[u8]) {
        assert_eq!(data.len(), self.def.byte_size);
        self.data.extend_from_slice(data);
    }

    pub fn len(&self) -> usize {
        self.data.len() / self.def.byte_size
    }

    pub fn remove(&mut self, index: usize) {
        let drop = self.def.drop;
        let range = self.get_index_range(index);
        unsafe {
            (drop)(self.data.as_mut_ptr().add(range.start));
        }
        self.data.drain(range);
    }

    pub fn swap_remove(&mut self, index: usize) {
        let drop = self.def.drop;
        let range = self.get_index_range(index);
        unsafe {
            (drop)(self.data.as_mut_ptr().add(range.start));
        }
        if index != self.len() - 1 {
            return;
        }

        // safe borrow checked version of a memcpy from the end of the array
        // to the location we just deleted
        let swap_from = self.get_index_range(self.len() - 1);
        let (left, right) = self.data.split_at_mut(swap_from.start);
        let from = &right[00..swap_from.len()];
        let too = &mut left[range];
        too.copy_from_slice(from);
    }

    pub fn clear(&mut self) {
        let drop = self.def.drop;
        for index in 0..self.len() {
            let offset = index * self.def.byte_size;
            unsafe {
                (drop)(self.data.as_mut_ptr().add(offset));
            }
        }
        self.data.clear();
    }
}

impl Drop for MetaValueVec {
    fn drop(&mut self) {}
}

pub struct MetaValueQueue {
    data: Vec<u8>,
    def: &'static MetaTypeDefinition,
}

impl<T: MetaType> MetaType for Vec<T> {
    fn meta_id() -> MetaTypeId {
        MetaTypeId(global_unique_usize!())
    }

    fn meta_def() -> &'static MetaTypeDefinition {
        static DEF: OnceLock<MetaTypeDefinition> = OnceLock::new();
        DEF.get_or_init(|| MetaTypeDefinition {
            id: Self::meta_id(),
            ident: std::any::type_name::<Self>(),
            byte_size: std::mem::size_of::<Self>(),
            fields: &[],
            drop: make_drop_fn::<Self>(),
        })
    }
}

pub fn make_drop_fn<T>() -> unsafe fn(*mut u8) {
    unsafe fn drop_impl<T>(ptr: *mut u8) {
        unsafe { std::ptr::drop_in_place(ptr as *mut T) };
    }
    drop_impl::<T>
}

pub struct StoreValueRef {
    pub ty: StoreValueTypeId,
    pub index: usize,
}

pub struct StoreValueTable {
    pub values: MetaValueVec,
    pub value_ids: Vec<StoreValueId>,
}

define_stable_key!(StoreValueTypeId);
define_stable_key!(StoreValueId);
/// Allocator and owner for meta values
#[derive(Default)]
pub struct MetaValueStore {
    values: StableTable<StoreValueTypeId, UnsafeCell<StoreValueTable>>,
    id_to_value: StableTable<StoreValueId, StoreValueRef>,
    def_to_type: HashMap<MetaTypeId, StoreValueTypeId>,
}

impl MetaValueStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add<T: MetaType>(&mut self, value: T) -> StoreValueId {
        let value_type_id = if let Some(value_type_id) = self.def_to_type.get(&T::meta_id()) {
            *value_type_id
        } else {
            let value_type_id = self.values.add(UnsafeCell::new(StoreValueTable {
                values: MetaValueVec::new(T::meta_def()),
                value_ids: Vec::new(),
            }));
            self.def_to_type.insert(T::meta_id(), value_type_id);
            value_type_id
        };
        let value_table = self.values[value_type_id].get_mut();

        let index = value_table.values.len();

        // Move value into value table
        value_table.values.push(value.as_bytes());
        std::mem::forget(value);

        let value_ref = StoreValueRef {
            ty: value_type_id,
            index,
        };
        let value_id = self.id_to_value.add(value_ref);

        value_id
    }

    pub fn get(&self, value: StoreValueId) -> &[u8] {
        let path = &self.id_to_value[value];
        unsafe { (*self.values[path.ty].get()).values.get(path.index) }
    }

    pub unsafe fn get_mut_unchecked(&self, value: StoreValueId) -> &mut [u8] {
        let path = &self.id_to_value[value];
        unsafe { (*self.values[path.ty].get()).values.get_mut(path.index) }
    }
}
