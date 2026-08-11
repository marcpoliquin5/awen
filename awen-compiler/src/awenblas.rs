use crate::ir::{Layout, Tensor};
use crate::lowering::Tile;
use anyhow::{bail, Result};

pub fn reference_gemm(
    lhs: &Tensor,
    rhs: &Tensor,
    transpose_lhs: bool,
    transpose_rhs: bool,
    m: usize,
    n: usize,
    k: usize,
) -> Result<Vec<f64>> {
    let lhs_data = lhs
        .data
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("tensor '{}' has no literal data", lhs.id))?;
    let rhs_data = rhs
        .data
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("tensor '{}' has no literal data", rhs.id))?;
    let mut output = vec![0.0; m * n];
    accumulate_tile(
        &mut output,
        lhs,
        rhs,
        lhs_data,
        rhs_data,
        transpose_lhs,
        transpose_rhs,
        Tile {
            m_offset: 0,
            n_offset: 0,
            k_offset: 0,
            m,
            n,
            k,
        },
        n,
    )?;
    Ok(output)
}

#[allow(clippy::too_many_arguments)]
pub fn accumulate_tile(
    output: &mut [f64],
    lhs: &Tensor,
    rhs: &Tensor,
    lhs_data: &[f64],
    rhs_data: &[f64],
    transpose_lhs: bool,
    transpose_rhs: bool,
    tile: Tile,
    output_columns: usize,
) -> Result<()> {
    if output_columns == 0 || !output.len().is_multiple_of(output_columns) {
        bail!("output buffer is incompatible with its declared column count");
    }
    for local_m in 0..tile.m {
        let row = tile.m_offset + local_m;
        for local_n in 0..tile.n {
            let column = tile.n_offset + local_n;
            let mut partial = 0.0;
            for local_k in 0..tile.k {
                let inner = tile.k_offset + local_k;
                let lhs_value = matrix_value(lhs, lhs_data, row, inner, transpose_lhs)?;
                let rhs_value = matrix_value(rhs, rhs_data, inner, column, transpose_rhs)?;
                partial += lhs_value * rhs_value;
            }
            output[row * output_columns + column] += partial;
        }
    }
    Ok(())
}

pub fn matrix_value(
    tensor: &Tensor,
    data: &[f64],
    row: usize,
    column: usize,
    transpose: bool,
) -> Result<f64> {
    let (physical_row, physical_column) = if transpose {
        (column, row)
    } else {
        (row, column)
    };
    if physical_row >= tensor.shape[0] || physical_column >= tensor.shape[1] {
        bail!(
            "index [{physical_row}, {physical_column}] is outside tensor '{}' shape {:?}",
            tensor.id,
            tensor.shape
        );
    }
    let index = match tensor.layout {
        Layout::RowMajor => physical_row * tensor.shape[1] + physical_column,
        Layout::ColumnMajor => physical_column * tensor.shape[0] + physical_row,
    };
    Ok(data[index])
}
