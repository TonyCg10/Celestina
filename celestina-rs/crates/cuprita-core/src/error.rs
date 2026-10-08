//! Typed failures of the three backends. Each turns into the Spanish sentence
//! the window's notice pill shows through `message_es()`; that sentence is
//! product copy, so this file is declared as such.
//!
//! language-contract: product-copy

macro_rules! section_error {
    ($name:ident, $service:literal) => {
        #[derive(Debug, Clone, PartialEq, Eq)]
        pub enum $name {
            /// The service is not running or cannot be reached.
            Unavailable,
            /// The service refused: policy or authorisation.
            Denied,
            /// The named object does not exist (any more).
            NotFound(String),
            /// Anything else, with the service's own detail.
            Failed(String),
        }

        impl $name {
            #[must_use]
            pub fn message_es(&self) -> String {
                match self {
                    Self::Unavailable => format!("{} no está disponible", $service),
                    Self::Denied => "No tienes permiso para hacer eso".to_owned(),
                    Self::NotFound(what) => format!("No se encuentra «{what}»"),
                    Self::Failed(detail) => format!("No se pudo completar: {detail}"),
                }
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{:?}", self)
            }
        }

        impl std::error::Error for $name {}
    };
}

section_error!(NetworkError, "La red");
section_error!(BluetoothError, "El Bluetooth");
section_error!(AudioError, "El sonido");
