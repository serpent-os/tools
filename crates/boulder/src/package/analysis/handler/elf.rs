use std::{fs::File, path::Path};

use elf::{
    abi::{DT_NEEDED, DT_SONAME},
    endian::AnyEndian,
    to_str,
};
use moss::{dependency, Dependency, Provider};

use crate::package::{
    analysis::{BoxError, BucketMut, Decision, Response},
    collect::PathInfo,
};

pub fn elf(bucket: &mut BucketMut, info: &mut PathInfo) -> Result<Response, BoxError> {
    let file_name = info.file_name();

    if file_name.ends_with(".debug") && info.has_component("debug") {
        return Ok(Decision::NextHandler.into());
    }
    if !info.is_file() {
        return Ok(Decision::NextHandler.into());
    }

    let Ok(mut elf) = parse(&info.path) else {
        return Ok(Decision::NextHandler.into());
    };

    let machine_isa = to_str::e_machine_to_str(elf.ehdr.e_machine)
        .and_then(|s| s.strip_prefix("EM_"))
        .unwrap_or_default()
        .to_lowercase();

    parse_dynamic_section(&mut elf, bucket, &machine_isa, file_name);

    Ok(Decision::IncludeFile.into())
}

fn parse(path: &Path) -> Result<elf::ElfStream<AnyEndian, File>, BoxError> {
    let file = File::open(path)?;
    Ok(elf::ElfStream::open_stream(file)?)
}

fn parse_dynamic_section(
    elf: &mut elf::ElfStream<AnyEndian, File>,
    bucket: &mut BucketMut,
    machine_isa: &str,
    file_name: &str,
) {
    let mut dt_needed_offsets = vec![];
    let mut soname_offset = None;

    // Get all dynamic entry offsets into string table
    if let Ok(Some(table)) = elf.dynamic() {
        for entry in table.iter() {
            match entry.d_tag {
                DT_NEEDED => {
                    dt_needed_offsets.push(entry.d_val() as usize);
                }
                DT_SONAME => {
                    soname_offset = Some(entry.d_val() as usize);
                }
                _ => {}
            }
        }
    }

    // Resolve offsets against string table and add the applicable
    // depends and provides
    if let Ok(Some((_, strtab))) = elf.dynamic_symbol_table() {
        // needed = dependency
        for offset in dt_needed_offsets {
            if let Ok(name) = strtab.get(offset) {
                bucket.dependencies.insert(Dependency {
                    kind: dependency::Kind::SharedLibary,
                    name: format!("{name}({machine_isa})"),
                });
            }
        }

        // soname exposed, let's share it
        if file_name.contains(".so") {
            let mut name = "";

            if let Some(offset) = soname_offset {
                if let Ok(val) = strtab.get(offset) {
                    name = val;
                }
            }

            if name.is_empty() {
                name = file_name;
            }

            bucket.providers.insert(Provider {
                kind: dependency::Kind::SharedLibary,
                name: format!("{name}({machine_isa})"),
            });
        }
    }
}
