use anyhow::{Context, Result};
use awen_compiler::{ExecutableCommand, ExecutablePackage};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedExecutable {
    pub backend_id: String,
    pub abi_major: u16,
    pub abi_minor: u16,
    pub dispatches: Vec<PreparedDispatch>,
    pub mlir_bytecode_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedDispatch {
    pub tile: [u32; 3],
    pub minimum_effective_bits: u16,
    pub calibration: String,
    pub layout: String,
    pub result_shape: Vec<i64>,
}

pub fn prepare_executable(bytes: &[u8]) -> Result<PreparedExecutable> {
    let package = ExecutablePackage::decode(bytes).context("invalid AWEN executable artifact")?;
    let dispatches = package
        .commands
        .into_iter()
        .map(|command| match command {
            ExecutableCommand::ExecuteGemm {
                tile_m,
                tile_n,
                tile_k,
                minimum_effective_bits,
                calibration,
                layout,
                result_shape,
            } => PreparedDispatch {
                tile: [tile_m, tile_n, tile_k],
                minimum_effective_bits,
                calibration,
                layout,
                result_shape,
            },
        })
        .collect();
    Ok(PreparedExecutable {
        backend_id: package.backend_id,
        abi_major: package.abi_major,
        abi_minor: package.abi_minor,
        dispatches,
        mlir_bytecode_bytes: package.mlir_bytecode.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_non_executable_input() {
        let error = prepare_executable(b"not an executable").unwrap_err();
        assert!(error
            .to_string()
            .contains("invalid AWEN executable artifact"));
    }
}
