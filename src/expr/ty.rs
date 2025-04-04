use std::fmt::Debug;

use stable_mir::CrateDef;
use stable_mir::mir::*;
use stable_mir::ty::*;

use crate::symbol::nstring::NString;

pub type FieldDef = (NString, Type);
pub type StructDef = (NString, Vec<FieldDef>);
pub type TupleDef = Vec<Type>;
pub type FunctionDef = (FnDef, GenericArgs);

/// A wrapper for `Ty` in MIR
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Type(Ty);

impl Type {
    pub fn unit_type() -> Self {
        Type::from(Ty::new_tuple(&[]))
    }

    pub fn bool_type() -> Self {
        Type::from(Ty::bool_ty())
    }

    pub fn signed_type(ty: IntTy) -> Self {
        Type::from(Ty::signed_ty(ty))
    }

    pub fn isize_type() -> Self {
        Type::signed_type(IntTy::Isize)
    }

    pub fn unsigned_type(ty: UintTy) -> Self {
        Type::from(Ty::unsigned_ty(ty))
    }

    pub fn usize_type() -> Self {
        Type::unsigned_type(UintTy::Usize)
    }

    pub fn array_type(elem_ty: Type, len: u64) -> Self {
        Type::from(
            Ty::try_new_array(elem_ty.0, len)
                .expect(format!("({elem_ty:?}, {len}) is wrong for an array type").as_str()),
        )
    }

    pub fn infinite_array_type(elem_ty: Type) -> Self {
        // Array with len 0 as const array type
        Type::array_type(elem_ty, 0)
    }

    pub fn slice_type(elem_ty: Type) -> Self {
        Type::from(Ty::from_rigid_kind(RigidTy::Slice(elem_ty.0)))
    }

    pub fn slice_type_from_array_type(array_type: Type) -> Self {
        assert!(array_type.is_array());
        Type::slice_type(array_type.elem_type())
    }

    pub fn ptr_type(pointee_type: Type, m: Mutability) -> Self {
        Type::from(Ty::new_ptr(pointee_type.0, m))
    }

    pub fn ref_type(reg: Region, pointee_type: Type, mutability: Mutability) -> Self {
        Type::from(Ty::new_ref(reg, pointee_type.0, mutability))
    }

    pub fn box_type(inner_type: Type) -> Self {
        Ty::new_box(inner_type.0).into()
    }

    pub fn is_unit(&self) -> bool {
        self.0.kind().is_unit()
    }

    pub fn is_bool(&self) -> bool {
        self.0.kind().is_bool()
    }

    pub fn is_signed(&self) -> bool {
        self.0.kind().is_signed()
    }

    pub fn is_isize(&self) -> bool {
        *self == Type::isize_type()
    }

    pub fn is_unsigned(&self) -> bool {
        self.0.kind().is_integral() && !self.is_signed()
    }

    pub fn is_usize(&self) -> bool {
        *self == Type::usize_type()
    }

    pub fn is_integer(&self) -> bool {
        self.0.kind().is_integral()
    }

    pub fn is_primitive(&self) -> bool {
        self.0.kind().is_primitive()
    }

    pub fn is_enum(&self) -> bool {
        self.0.kind().is_enum()
    }

    pub fn is_array(&self) -> bool {
        self.0.kind().is_array()
    }

    pub fn is_slice(&self) -> bool {
        self.0.kind().is_slice()
    }

    pub fn is_fn(&self) -> bool {
        self.0.kind().is_fn()
    }

    pub fn is_layout(&self) -> bool {
        self.name() == "Layout"
    }

    pub fn is_struct(&self) -> bool {
        self.0.kind().is_struct() && !self.is_box() && !self.is_layout()
    }

    pub fn is_tuple(&self) -> bool {
        match self.0.kind().rigid() {
            Some(r) => matches!(r, RigidTy::Tuple(..)),
            None => false,
        }
    }

    pub fn is_ref(&self) -> bool {
        self.0.kind().is_ref()
    }

    pub fn is_ptr(&self) -> bool {
        self.0.kind().is_raw_ptr()
    }

    pub fn is_box(&self) -> bool {
        self.0.kind().is_box()
    }

    pub fn is_slice_ptr(&self) -> bool {
        self.is_any_ptr() && self.pointee_ty().is_slice()
    }

    /// `Box` is also a ptr by our semantic
    pub fn is_any_ptr(&self) -> bool {
        self.is_ref() || self.is_ptr() || self.is_box()
    }

    pub fn is_primitive_ptr(&self) -> bool {
        self.is_ptr() || self.is_ref()
    }

    /// Size will be in field-level
    pub fn num_fields(&self) -> Option<usize> {
        if self.is_unit() {
            return Some(0);
        }
        if self.is_bool() || self.is_integer() || self.is_any_ptr() {
            return Some(1);
        }

        if self.is_array() {
            return Some(self.array_size().unwrap() as usize);
        }

        if self.is_struct() {
            let def = self.struct_def();
            let size = def.1.iter().fold(0, |acc, x| {
                acc + match x.1.num_fields() {
                    Some(s) => s,
                    None => panic!("Impoissible"),
                }
            });
            return Some(size);
        }

        if self.is_tuple() {
            let def = self.tuple_def();
            let size = def.iter().fold(0, |acc, x| {
                acc + match x.num_fields() {
                    Some(s) => s,
                    None => panic!("Impoissible"),
                }
            });
            return Some(size);
        }

        todo!("{self:?}")
    }

    pub fn pointee_ty(&self) -> Self {
        assert!(self.is_any_ptr());
        match self.0.kind() {
            TyKind::RigidTy(r) => {
                match r {
                    RigidTy::Adt(def, args) => {
                        assert!(def.is_box());
                        // TODO: handle args more carefully
                        match &args.0[0] {
                            GenericArgKind::Type(ty) => Type::from(ty.clone()),
                            _ => panic!(),
                        }
                    }
                    RigidTy::RawPtr(ty, ..) | RigidTy::Ref(_, ty, ..) => Type::from(ty),
                    _ => panic!(),
                }
            }
            _ => panic!(),
        }
    }

    pub fn array_size(&self) -> Option<u64> {
        assert!(self.is_array());
        let size = match self.0.kind() {
            TyKind::RigidTy(r) => match r {
                RigidTy::Array(_, c) => c.eval_target_usize(),
                _ => panic!("Not array"),
            },
            _ => panic!("Not array"),
        }
        .expect("Wrong array size");
        if size == 0 { None } else { Some(size) }
    }

    /// Assume that all index is integer.
    pub fn array_domain(&self) -> Type {
        assert!(self.is_array());
        Type::unsigned_type(UintTy::Usize)
    }

    /// Range for array/slice
    pub fn elem_type(&self) -> Type {
        assert!(self.is_array() || self.is_slice());
        if let TyKind::RigidTy(r) = self.0.kind() {
            return match r {
                RigidTy::Array(t, _) | RigidTy::Slice(t) => Type::from(t),
                _ => panic!("Impossible"),
            };
        }
        panic!("Impossible")
    }

    pub fn struct_def(&self) -> StructDef {
        assert!(self.is_struct());
        let mut def = (self.name(), Vec::new());
        if let TyKind::RigidTy(r) = self.0.kind() {
            if let RigidTy::Adt(adt, args) = r {
                for field in adt.variants()[0].fields() {
                    let fty = field.ty_with_args(&args);
                    def.1.push((NString::from(field.name.clone()), Type::from(fty)));
                }
            }
        }
        assert!(!def.1.is_empty());
        def
    }

    pub fn tuple_def(&self) -> TupleDef {
        match self.0.kind().rigid() {
            Some(r) => match r {
                RigidTy::Tuple(fields) => fields.iter().map(|t| Type::from(t)).collect::<Vec<_>>(),
                _ => panic!("Not tuple"),
            },
            None => panic!("Not tuple"),
        }
    }

    pub fn fn_def(&self) -> FunctionDef {
        assert!(self.is_fn());
        let kind = self.0.kind();
        let _def = kind.fn_def().unwrap();
        (_def.0, _def.1.clone())
    }

    pub fn name(&self) -> NString {
        match self.0.kind().rigid().unwrap() {
            RigidTy::Bool => "bool".into(),
            RigidTy::Char => "char".into(),
            RigidTy::Int(i) => format!("{i:?}").to_lowercase().into(),
            RigidTy::Uint(i) => format!("{i:?}").to_lowercase().into(),
            RigidTy::Adt(def, ..) => def.trimmed_name().into(),
            RigidTy::Array(ty, ..) => format!("Array({:?})", Type(*ty).name()).into(),
            RigidTy::Slice(ty) => format!("Slice({:?})", Type(*ty).name()).into(),
            RigidTy::RawPtr(ty, ..) => format!("Ptr({:?})", Type(*ty).name()).into(),
            RigidTy::Ref(_, ty, _) => format!("Ref({:?})", Type(*ty).name()).into(),
            RigidTy::Never => "never".into(),
            RigidTy::Tuple(f) => {
                if f.is_empty() {
                    "unit".into()
                } else {
                    let mut name = NString::from("_tuple");
                    for ty in f {
                        name += format!("_{:?}", Type(*ty).name());
                    }
                    name
                }
            }
            _ => todo!(),
        }
    }
}

impl Debug for Type {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<Ty> for Type {
    fn from(value: Ty) -> Self {
        assert!(matches!(value.kind(), TyKind::RigidTy(_)));
        Type(value)
    }
}

impl From<&Ty> for Type {
    fn from(value: &Ty) -> Self {
        Type::from(*value)
    }
}
