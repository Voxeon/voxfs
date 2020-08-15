pub trait ByteSerializable {
    type BytesArrayType;

    fn to_bytes(&self) -> Self::BytesArrayType;
    fn from_bytes(bytes: &[u8]) -> Option<Self>
    where
        Self: core::marker::Sized;
    fn generic_bytes_rep(bytes: &Self::BytesArrayType) -> &[u8];
}
