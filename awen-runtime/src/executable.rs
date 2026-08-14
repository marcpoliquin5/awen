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
    use awen_compiler::executable::{EXECUTABLE_ABI_MAJOR, EXECUTABLE_ABI_MINOR, EXECUTABLE_MAGIC};

    fn rank_three_package() -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(EXECUTABLE_MAGIC);
        bytes.extend_from_slice(&EXECUTABLE_ABI_MAJOR.to_le_bytes());
        bytes.extend_from_slice(&EXECUTABLE_ABI_MINOR.to_le_bytes());
        let backend = b"awen.reference.v1";
        bytes.extend_from_slice(&(backend.len() as u16).to_le_bytes());
        bytes.extend_from_slice(backend);
        bytes.extend_from_slice(&1_u32.to_le_bytes());
        bytes.push(1);
        bytes.extend_from_slice(&128_u32.to_le_bytes());
        bytes.extend_from_slice(&64_u32.to_le_bytes());
        bytes.extend_from_slice(&32_u32.to_le_bytes());
        bytes.extend_from_slice(&8_u16.to_le_bytes());
        let calibration = b"required";
        bytes.extend_from_slice(&(calibration.len() as u16).to_le_bytes());
        bytes.extend_from_slice(calibration);
        let layout = b"row_major";
        bytes.extend_from_slice(&(layout.len() as u16).to_le_bytes());
        bytes.extend_from_slice(layout);
        bytes.push(3);
        for dimension in [2_i64, 16, 8] {
            bytes.extend_from_slice(&dimension.to_le_bytes());
        }
        let bytecode = b"ML\xefRtest";
        bytes.extend_from_slice(&(bytecode.len() as u32).to_le_bytes());
        bytes.extend_from_slice(bytecode);
        bytes
    }

    #[test]
    fn rejects_non_executable_input() {
        let error = prepare_executable(b"not an executable").unwrap_err();
        assert!(error
            .to_string()
            .contains("invalid AWEN executable artifact"));
    }

    #[test]
    fn prepares_equal_batch_rank_three_dispatch() {
        let executable = prepare_executable(&rank_three_package()).unwrap();
        assert_eq!(executable.dispatches.len(), 1);
        assert_eq!(executable.dispatches[0].result_shape, vec![2, 16, 8]);
        assert_eq!(executable.dispatches[0].tile, [128, 64, 32]);
    }
}
