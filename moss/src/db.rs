// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

pub use self::encoding::{Decoder, Encoding};

pub mod meta;
pub mod state;

mod encoding {
    //! Decode from sql types to rust types
    use std::convert::Infallible;

    use sqlx::{Sqlite, Type};

    use crate::registry::package;
    use crate::{dependency, Dependency, Provider};

    /// Decode from a database type using [`Encoding::decode`]
    #[derive(Debug, Clone, Copy)]
    pub struct Decoder<T>(pub T);

    /// A trait to define an encoding between a sql type and rust type
    pub trait Encoding: Sized {
        type Encoded;
        type Error;

        fn decode(encoded: Self::Encoded) -> Result<Self, Self::Error>;
        fn encode(self) -> Self::Encoded;
    }

    impl<'r, T, U, E> sqlx::Decode<'r, Sqlite> for Decoder<T>
    where
        T: Encoding<Encoded = U, Error = E>,
        U: sqlx::Decode<'r, Sqlite>,
        E: std::error::Error + Send + Sync + 'static,
    {
        fn decode(
            value: <Sqlite as sqlx::database::HasValueRef<'r>>::ValueRef,
        ) -> Result<Self, sqlx::error::BoxDynError> {
            Ok(T::decode(U::decode(value)?).map(Decoder)?)
        }
    }

    impl<T, U, E> Type<Sqlite> for Decoder<T>
    where
        T: Encoding<Encoded = U, Error = E>,
        U: Type<Sqlite>,
    {
        fn type_info() -> <Sqlite as sqlx::Database>::TypeInfo {
            U::type_info()
        }

        fn compatible(ty: &<Sqlite as sqlx::Database>::TypeInfo) -> bool {
            U::compatible(ty)
        }
    }

    /** Encoding on external types */

    /// Encoding of package identity (String)
    impl Encoding for package::Id {
        type Encoded = String;
        type Error = Infallible;

        fn decode(encoded: Self::Encoded) -> Result<Self, Self::Error> {
            Ok(package::Id::from(encoded))
        }

        fn encode(self) -> Self::Encoded {
            String::from(self)
        }
    }

    /// Encoding of Dependency type
    impl Encoding for Dependency {
        type Encoded = String;
        type Error = dependency::ParseError;

        fn decode(encoded: Self::Encoded) -> Result<Self, Self::Error> {
            encoded.parse()
        }

        fn encode(self) -> Self::Encoded {
            self.to_string()
        }
    }

    /// Encoding of Provider type
    impl Encoding for Provider {
        type Encoded = String;
        type Error = dependency::ParseError;

        fn decode(encoded: Self::Encoded) -> Result<Self, Self::Error> {
            encoded.parse()
        }

        fn encode(self) -> Self::Encoded {
            self.to_string()
        }
    }
}
