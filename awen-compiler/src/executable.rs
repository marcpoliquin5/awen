use anyhow::{bail, Context, Result};

pub const EXECUTABLE_MAGIC: &[u8; 8] = b"AWENEXE\0";
pub const EXECUTABLE_ABI_MAJOR: u16 = 1;
pub const EXECUTABLE_ABI_MINOR: u16 = 0;
const EXECUTE_GEMM_KIND: u8 = 1;
const MLIR_BYTECODE_MAGIC: &[u8; 4] = b"ML\xefR";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutableCommand {
    ExecuteGemm {
        tile_m: u32,
        tile_n: u32,
        tile_k: u32,
        minimum_effective_bits: u16,
        calibration: String,
        layout: String,
        result_shape: Vec<i64>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutablePackage {
    pub abi_major: u16,
    pub abi_minor: u16,
    pub backend_id: String,
    pub commands: Vec<ExecutableCommand>,
    pub mlir_bytecode: Vec<u8>,
}

impl ExecutablePackage {
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(bytes);
        let magic = reader.read_exact(EXECUTABLE_MAGIC.len())?;
        if magic != EXECUTABLE_MAGIC {
            bail!("invalid AWEN executable magic");
        }
        let abi_major = reader.read_u16()?;
        let abi_minor = reader.read_u16()?;
        if abi_major != EXECUTABLE_ABI_MAJOR {
            bail!(
                "unsupported AWEN executable ABI {abi_major}.{abi_minor}; runtime supports {}.x",
                EXECUTABLE_ABI_MAJOR
            );
        }
        let backend_id = reader.read_string("backend identifier")?;
        if backend_id.is_empty() {
            bail!("AWEN executable backend identifier is empty");
        }
        let command_count = reader.read_u32()? as usize;
        let mut commands = Vec::with_capacity(command_count);
        for index in 0..command_count {
            let kind = reader.read_u8()?;
            match kind {
                EXECUTE_GEMM_KIND => {
                    let tile_m = reader.read_u32()?;
                    let tile_n = reader.read_u32()?;
                    let tile_k = reader.read_u32()?;
                    let minimum_effective_bits = reader.read_u16()?;
                    let calibration = reader
                        .read_string("calibration handle")
                        .with_context(|| format!("command {index}"))?;
                    let layout = reader
                        .read_string("tensor layout")
                        .with_context(|| format!("command {index}"))?;
                    if layout != "row_major" && layout != "column_major" {
                        bail!("command {index} has unsupported tensor layout '{layout}'");
                    }
                    let rank = reader.read_u8()? as usize;
                    if rank == 0 || rank > 8 {
                        bail!("command {index} has unsupported result rank {rank}");
                    }
                    let mut result_shape = Vec::with_capacity(rank);
                    for _ in 0..rank {
                        let dimension = reader.read_i64()?;
                        if dimension == 0 || dimension < -1 {
                            bail!("command {index} has invalid result dimension {dimension}");
                        }
                        result_shape.push(dimension);
                    }
                    if tile_m == 0 || tile_n == 0 || tile_k == 0 {
                        bail!("command {index} has a zero tile dimension");
                    }
                    if minimum_effective_bits == 0 {
                        bail!("command {index} has a zero-bit precision contract");
                    }
                    commands.push(ExecutableCommand::ExecuteGemm {
                        tile_m,
                        tile_n,
                        tile_k,
                        minimum_effective_bits,
                        calibration,
                        layout,
                        result_shape,
                    });
                }
                _ => bail!("command {index} has unknown command kind {kind}"),
            }
        }
        if commands.is_empty() {
            bail!("AWEN executable contains no device commands");
        }
        let bytecode_size = reader.read_u32()? as usize;
        let mlir_bytecode = reader.read_exact(bytecode_size)?.to_vec();
        if !mlir_bytecode.starts_with(MLIR_BYTECODE_MAGIC) {
            bail!("AWEN executable does not contain valid MLIR bytecode");
        }
        if !reader.is_empty() {
            bail!("AWEN executable has trailing bytes");
        }
        Ok(Self {
            abi_major,
            abi_minor,
            backend_id,
            commands,
            mlir_bytecode,
        })
    }
}

struct Reader<'a> {
    remaining: &'a [u8],
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { remaining: bytes }
    }

    fn read_exact(&mut self, length: usize) -> Result<&'a [u8]> {
        if self.remaining.len() < length {
            bail!("truncated AWEN executable");
        }
        let (value, remaining) = self.remaining.split_at(length);
        self.remaining = remaining;
        Ok(value)
    }

    fn read_u8(&mut self) -> Result<u8> {
        Ok(self.read_exact(1)?[0])
    }

    fn read_u16(&mut self) -> Result<u16> {
        Ok(u16::from_le_bytes(self.read_exact(2)?.try_into()?))
    }

    fn read_u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.read_exact(4)?.try_into()?))
    }

    fn read_i64(&mut self) -> Result<i64> {
        Ok(i64::from_le_bytes(self.read_exact(8)?.try_into()?))
    }

    fn read_string(&mut self, field: &str) -> Result<String> {
        let length = self.read_u16()? as usize;
        let value = std::str::from_utf8(self.read_exact(length)?)
            .with_context(|| format!("{field} is not valid UTF-8"))?;
        Ok(value.to_string())
    }

    fn is_empty(&self) -> bool {
        self.remaining.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_package() -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(EXECUTABLE_MAGIC);
        bytes.extend_from_slice(&EXECUTABLE_ABI_MAJOR.to_le_bytes());
        bytes.extend_from_slice(&EXECUTABLE_ABI_MINOR.to_le_bytes());
        let backend = b"awen.reference.v1";
        bytes.extend_from_slice(&(backend.len() as u16).to_le_bytes());
        bytes.extend_from_slice(backend);
        bytes.extend_from_slice(&1_u32.to_le_bytes());
        bytes.push(EXECUTE_GEMM_KIND);
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
        bytes.push(2);
        bytes.extend_from_slice(&256_i64.to_le_bytes());
        bytes.extend_from_slice(&64_i64.to_le_bytes());
        let bytecode = b"ML\xefRtest";
        bytes.extend_from_slice(&(bytecode.len() as u32).to_le_bytes());
        bytes.extend_from_slice(bytecode);
        bytes
    }

    #[test]
    fn decodes_versioned_executable() {
        let package = ExecutablePackage::decode(&valid_package()).unwrap();
        assert_eq!(package.abi_major, 1);
        assert_eq!(package.backend_id, "awen.reference.v1");
        assert_eq!(package.commands.len(), 1);
        assert_eq!(package.mlir_bytecode, b"ML\xefRtest");
    }

    #[test]
    fn rejects_truncated_executable() {
        let mut bytes = valid_package();
        bytes.pop();
        assert!(ExecutablePackage::decode(&bytes)
            .unwrap_err()
            .to_string()
            .contains("truncated"));
    }

    #[test]
    fn rejects_future_major_abi() {
        let mut bytes = valid_package();
        bytes[8..10].copy_from_slice(&2_u16.to_le_bytes());
        assert!(ExecutablePackage::decode(&bytes)
            .unwrap_err()
            .to_string()
            .contains("unsupported AWEN executable ABI"));
    }

    #[test]
    fn rejects_unknown_layout() {
        let mut bytes = valid_package();
        let start = bytes
            .windows(b"row_major".len())
            .position(|window| window == b"row_major")
            .unwrap();
        bytes[start..start + b"row_major".len()].copy_from_slice(b"bad_major");
        assert!(ExecutablePackage::decode(&bytes)
            .unwrap_err()
            .to_string()
            .contains("unsupported tensor layout"));
    }
}
