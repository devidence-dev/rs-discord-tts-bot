//! Lógica portable del bot TTS (port Go → Rust).
//!
//! Cada módulo es un puerto de su contraparte en `bot-go/internal/`, según
//! ../bot-go/plan-go-rust.md. Vive en una crate de biblioteca — separada del
//! binario en `main.rs` — para que los tipos públicos de cada fase queden
//! disponibles para tests de integración y para `main` sin que el compilador
//! se queje de "código muerto" mientras el wiring del bot (fase 8) todavía no
//! los usa.

pub mod data;
pub mod locale;
pub mod provider;
pub mod settings;
pub mod player;
pub mod commands;





