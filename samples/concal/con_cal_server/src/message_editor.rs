use protobuf::descriptor::FileDescriptorSet;
use protobuf::reflect::MessageDescriptor;
use protobuf::reflect::{FileDescriptor, ReflectValueBox, RuntimeFieldType};
use protobuf::well_known_types::any::Any;
use protobuf::MessageDyn;

#[derive(Debug)]
pub struct MessageEditor {
    msg_descriptor: MessageDescriptor,
    dyn_msg: Box<dyn MessageDyn>,
}

impl MessageEditor {
    pub fn new(data: Any, fds: FileDescriptorSet) -> Result<Self, String> {
        let msg_descriptor = match MessageEditor::find_message_descriptor(&data, &fds) {
            Ok(x) => x,
            Err(x) => return Err(x),
        };
        let dyn_msg = match MessageEditor::unpack_message(&data, &msg_descriptor) {
            Ok(x) => x,
            Err(x) => return Err(x),
        };
        Ok(MessageEditor { msg_descriptor, dyn_msg })
    }

    pub fn to_any(&self) -> Result<Any, String> {
        match protobuf::well_known_types::any::Any::pack_dyn(self.dyn_msg.as_ref()) {
            Ok(x) => Ok(x),
            Err(e) => Err(format!("Conversion to any failed: {}", e)),
        }
    }

    pub fn update_value(&mut self, key: &str, value: ReflectValueBox) -> Result<(), String> {
        let field = match self.msg_descriptor.field_by_name(key) {
            Some(x) => x,
            None => return Err(format!("Could not find field {}", key).to_string()),
        };

        match field.runtime_field_type() {
            RuntimeFieldType::Singular(_) => field.set_singular_field(self.dyn_msg.as_mut(), value),
            _ => return Err(format!("Unsupported runtime field type {:?}", key)),
        }

        Ok(())
    }

    pub fn unpack_message(
        data: &Any,
        message_descriptor: &MessageDescriptor,
    ) -> Result<Box<dyn MessageDyn>, String> {
        let dyn_msg = match data.unpack_dyn(message_descriptor) {
            Ok(x) => x,
            Err(e) => {
                return Err(format!("Failed to unpack dynamic message from data {}", e).to_string())
            }
        };
        let dyn_msg = match dyn_msg {
            Some(x) => x,
            None => return Err("Dynamic message contained none value".to_string()),
        };
        Ok(dyn_msg)
    }

    pub fn find_message_descriptor(
        data: &Any,
        metadata: &FileDescriptorSet,
    ) -> Result<MessageDescriptor, String> {
        let file_descriptors = match FileDescriptor::new_dynamic_fds(metadata.file.clone(), &[]) {
            Ok(x) => x,
            Err(e) => return Err(format!("Failed to create file descriptors: {}", e).to_string()),
        };
        for fd in file_descriptors {
            for msg in fd.messages() {
                if data.type_url.contains(msg.full_name()) {
                    return Ok(msg);
                }
            }
        }
        Err("Could not find message descriptor".to_string())
    }
}
