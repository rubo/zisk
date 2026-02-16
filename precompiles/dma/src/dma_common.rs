use fields::PrimeField64;
use zisk_pil::{
    Dma64AlignedInputCpyTrace, Dma64AlignedMemCpyTrace, Dma64AlignedMemSetTrace,
    Dma64AlignedMemTrace, Dma64AlignedTrace, DmaInputCpyTrace, DmaMemCpyTrace,
    DmaPrePostInputCpyTrace, DmaPrePostMemCpyTrace, DmaPrePostTrace, DmaTrace, DmaUnalignedTrace,
};

pub fn get_dma_air_name<F: PrimeField64>(air_id: usize) -> &'static str {
    match air_id {
        DmaTrace::<F>::AIR_ID => "Dma",
        DmaMemCpyTrace::<F>::AIR_ID => "DmaMemCpy",
        DmaInputCpyTrace::<F>::AIR_ID => "DmaInputCpy",
        DmaPrePostTrace::<F>::AIR_ID => "DmaPrePost",
        DmaPrePostMemCpyTrace::<F>::AIR_ID => "DmaPrePostMemCpy",
        DmaPrePostInputCpyTrace::<F>::AIR_ID => "DmaPrePostInputCpy",
        Dma64AlignedTrace::<F>::AIR_ID => "Dma64Aligned",
        Dma64AlignedMemSetTrace::<F>::AIR_ID => "Dma64AlignedMemSet",
        Dma64AlignedMemCpyTrace::<F>::AIR_ID => "Dma64AlignedMemCpy",
        Dma64AlignedInputCpyTrace::<F>::AIR_ID => "Dma64AlignedInputCpy",
        Dma64AlignedMemTrace::<F>::AIR_ID => "Dma64AlignedMem",
        DmaUnalignedTrace::<F>::AIR_ID => "DmaUnaligned",
        _ => "Unknown",
    }
}

pub fn dma_trace(title: &str, rows: usize, num_rows: usize) {
    if rows == num_rows {
        tracing::debug!("··· Creating {title} instance [{rows} / {rows} rows filled 100%]");
    } else {
        tracing::info!(
            "··· Creating {title} instance [{rows} / {num_rows} rows filled {:.2}%]",
            rows as f64 / num_rows as f64 * 100.0
        );
    }
}
