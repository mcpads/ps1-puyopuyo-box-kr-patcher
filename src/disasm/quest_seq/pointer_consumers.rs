//! QUEST SEQ의 opcode 역할군별 pointer consumer 증거를 조정한다.

mod callback_source;
mod deferred_streams;
mod fixed_render_record;
mod nested_render_records;
mod object_streams;
mod resident_async_sources;
mod table_records;

use crate::coords::ExeMap;
use crate::isa::Instruction;

use super::QuestSeqPointerConsumerError;

pub(super) fn verify_additional_pointer_consumers(
    instructions: &[Instruction],
    map: &ExeMap,
) -> Result<(), QuestSeqPointerConsumerError> {
    object_streams::verify(instructions, map)?;
    fixed_render_record::verify(instructions, map)?;
    table_records::verify(instructions, map)?;
    nested_render_records::verify(instructions, map)?;
    deferred_streams::verify(instructions, map)?;
    callback_source::verify(instructions, map)?;
    resident_async_sources::verify(instructions, map)
}
