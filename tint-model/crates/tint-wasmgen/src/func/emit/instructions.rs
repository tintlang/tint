use super::*;

impl<'a, 'b> Fc<'a, 'b> {
    pub(super) fn instr(&mut self, ins: &Instr) -> Result<(), Unsupported> {
        match ins {
            Instr::Get { .. }
            | Instr::Take { .. }
            | Instr::Set { .. }
            | Instr::Tag { .. }
            | Instr::Payload { .. }
            | Instr::Rt { .. }
            | Instr::Call { .. }
            | Instr::Closure { .. }
            | Instr::CallClosure { .. }
            | Instr::GlobalGet { .. }
            | Instr::GlobalSet { .. }
            | Instr::Map { .. } => self.instr_objects(ins),
            Instr::Host { .. }
            | Instr::UiOpen { .. }
            | Instr::UiClose
            | Instr::UiTokens { .. }
            | Instr::UiMemo { .. }
            | Instr::UiMemoEnd
            | Instr::UiText { .. } => self.instr_host(ins),
            _ => self.instr_basic(ins),
        }
    }
}
