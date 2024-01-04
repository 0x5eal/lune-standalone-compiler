use std::{
    collections::VecDeque,
    env,
    ops::ControlFlow,
    process::{self, ExitCode},
    sync::Mutex,
};

use lune::Lune;

use anyhow::Result;
use rayon::{
    iter::{IndexedParallelIterator, ParallelIterator},
    slice::ParallelSlice,
};
use tokio::fs::read as read_to_vec;

/**
    Returns information about whether the execution environment is standalone
    or not, the standalone binary signature, and the contents of the binary.
*/
pub async fn check_env() -> (bool, Vec<u8>, Vec<u8>) {
    // Signature which is only present in standalone lune binaries
    let signature: Vec<u8> = vec![0x4f, 0x3e, 0xf8, 0x41, 0xc3, 0x3a, 0x52, 0x16];

    // Read the current lune binary to memory
    let bin = if let Ok(contents) = read_to_vec(
        env::current_exe().expect("failed to get path to current running lune executable"),
    )
    .await
    {
        contents
    } else {
        Vec::new()
    };

    let is_standalone = !bin.is_empty() && bin[bin.len() - signature.len()..bin.len()] == signature;

    (is_standalone, signature, bin)
}

/**
    Discovers, loads and executes the bytecode contained in a standalone binary.
*/
pub async fn run_standalone(signature: Vec<u8>, bin: Vec<u8>) -> Result<ExitCode> {
    let bytecode_offset = Mutex::new(0);
    let bytecode_size = Mutex::new(0);

    // standalone binary structure (reversed, 8 bytes per field)
    // [0] => signature
    // ----------------
    // -- META Chunk --
    // [1] => file count
    // [2] => bytecode size
    // [3] => bytecode offset
    // ----------------
    // -- MISC Chunk --
    // [4..n] => bytecode (variable size)
    // ----------------
    // NOTE: All integers are 8 byte unsigned 64 bit (u64's).

    // The rchunks will have unequally sized sections in the beginning
    // but that doesn't matter to us because we don't need anything past the
    // middle chunks where the bytecode is stored
    bin.par_rchunks(signature.len())
        .enumerate()
        .try_for_each(|(idx, chunk)| {
            let mut bytecode_offset = bytecode_offset.lock().unwrap();
            let mut bytecode_size = bytecode_size.lock().unwrap();

            if *bytecode_offset != 0 && *bytecode_size != 0 {
                return ControlFlow::Break(());
            }

            if idx == 0 && chunk != signature {
                // Binary is guaranteed to be standalone, we've confirmed this before
                unreachable!("expected proper signature for standalone binary")
            }

            if idx == 3 {
                *bytecode_offset = u64::from_ne_bytes(chunk.try_into().unwrap());
            }

            if idx == 2 {
                *bytecode_size = u64::from_ne_bytes(chunk.try_into().unwrap());
            }

            ControlFlow::Continue(())
        });

    let bytecode_offset_inner = bytecode_offset.into_inner().unwrap();
    let bytecode_size_inner = bytecode_size.into_inner().unwrap();

    // If we were able to retrieve the required metadata, we load
    // and execute the bytecode

    let mut args = env::args().collect::<VecDeque<String>>();
    args.pop_front();

    let result = Lune::new()
        .with_args(args)
        .run(
            "STANDALONE",
            &bin[usize::try_from(bytecode_offset_inner)?
                ..usize::try_from(bytecode_offset_inner + bytecode_size_inner)?],
        )
        .await;

    Ok(match result {
        Err(err) => {
            eprintln!("{err}");
            ExitCode::FAILURE
        }
        Ok(code) => code,
    })
}
