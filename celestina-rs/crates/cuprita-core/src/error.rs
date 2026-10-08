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

impl NetworkError {
    /// An 802.1X network: Cuprita joins only open and passphrase networks;
    /// an enterprise profile is set up elsewhere.
    #[must_use]
    pub fn enterprise_unsupported() -> Self {
        Self::Failed("las redes empresariales se configuran fuera de Cuprita".to_owned())
    }

    /// A protected network without a saved profile was asked to join with no
    /// passphrase.
    #[must_use]
    pub fn password_required() -> Self {
        Self::Failed("esta red necesita una contraseña".to_owned())
    }
}

impl NetworkError {
    /// A WEP-only network: its keys are not a WPA passphrase and Cuprita does
    /// not offer them.
    #[must_use]
    pub fn wep_unsupported() -> Self {
        Self::Failed("las redes WEP no se admiten".to_owned())
    }
}

impl BluetoothError {
    /// The device or the person cancelled or refused the pairing
    /// (`org.bluez.Error.AuthenticationCanceled` or `…Rejected`).
    #[must_use]
    pub fn pairing_cancelled() -> Self {
        Self::Failed("emparejamiento cancelado".to_owned())
    }
}

/// The notice for an adapter switched on while airplane mode keeps every
/// radio off.
#[must_use]
pub fn airplane_mode_message() -> String {
    "Modo avión activado".to_owned()
}

/// The notice for a join that NetworkManager gave up on: a wrong passphrase,
/// or a network that went away.
#[must_use]
pub fn join_failed_message(name: &str) -> String {
    format!("No se pudo conectar a «{name}»: contraseña incorrecta o red no disponible")
}
