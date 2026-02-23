# Contexto del Proyecto: Qore Protocol

## 1. Visión General
**Qore** es una librería de transporte de datos de alto rendimiento para Node.js desarrollada como un Native Addon. El objetivo es proporcionar una comunicación ultra-rápida y robusta utilizando el protocolo QUIC (vía la librería `quiche` de Cloudflare) y serialización binaria con FlatBuffers.

## 2. Stack Tecnológico
- **Core:** Rust (Edición 2021).
- **Puente:** N-API mediante el framework `napi-rs` (Targeting napi8).
- **Transporte:** Protocolo QUIC (sobre UDP) usando la crate `quiche`.
- **Asincronía:** Tokio para el manejo de I/O asíncrono en el lado de Rust.
- **Entorno de Compilación:** Windows (x86_64-pc-windows-msvc).

## 3. Arquitectura de Memoria
El proyecto prioriza estrategias **Zero-copy**. Se busca pasar datos entre el motor de Rust y el Event Loop de Node.js compartiendo buffers de memoria (ArrayBuffers) sin realizar copias adicionales para minimizar la latencia y el uso de CPU.

## 4. Estructura de Archivos
- `src/lib.rs`: Punto de entrada de las funciones exportadas de Rust hacia Node.js.
- `Cargo.toml`: Gestión de dependencias de Rust (napi, quiche, tokio).
- `index.js`: Wrapper de JavaScript que expone las funciones nativas.
- `package.json`: Configuración de scripts de compilación (`npm run build`).

## 5. Reglas de Desarrollo para Copilot
- Siempre que se sugiera código para el transporte de datos, priorizar tipos binarios (`Buffer`, `Uint8Array`) sobre JSON.
- Las funciones exportadas en `lib.rs` deben usar el macro `#[napi]`.
- Al manejar QUIC, asegurar que el Handshake y el control de flujo se manejen en hilos de Rust para no bloquear el hilo principal de Node.js.
- El estilo de programación en Rust debe ser seguro, evitando bloques `unsafe` a menos que sea estrictamente necesario para el rendimiento de Zero-copy.