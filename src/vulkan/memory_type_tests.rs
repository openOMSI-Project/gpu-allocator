use super::*;

fn memory_type(index: usize, flags: vk::MemoryPropertyFlags) -> MemoryType {
    MemoryType {
        memory_blocks: Vec::new(),
        memory_properties: flags,
        memory_type_index: index,
        heap_index: index,
        mappable: flags.contains(vk::MemoryPropertyFlags::HOST_VISIBLE),
        active_general_blocks: 0,
        buffer_device_address: false,
    }
}

fn requirements(bits: u32) -> vk::MemoryRequirements {
    vk::MemoryRequirements {
        size: 64,
        alignment: 16,
        memory_type_bits: bits,
    }
}

fn upload() -> vk::MemoryPropertyFlags {
    vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT
}

#[test]
fn rejecting_the_first_preferred_type_does_not_hide_the_next_one() {
    let flags = upload() | vk::MemoryPropertyFlags::DEVICE_LOCAL;
    let types = [
        memory_type(0, flags),
        memory_type(1, flags),
        memory_type(2, upload()),
    ];
    assert_eq!(
        find_compatible_memory_type(&types, &requirements(7), flags, |t| t.memory_type_index
            != 0),
        Some(1)
    );
}

#[test]
fn a_retry_uses_another_compatible_type() {
    let types = [memory_type(0, upload()), memory_type(1, upload())];
    assert_eq!(
        find_compatible_memory_type(&types, &requirements(3), upload(), |t| t.memory_type_index
            != 0),
        Some(1)
    );
}

#[test]
fn a_failed_type_is_not_selected_again_when_it_is_the_only_one() {
    let types = [memory_type(0, upload())];
    assert_eq!(
        find_compatible_memory_type(&types, &requirements(1), upload(), |t| t.memory_type_index
            != 0),
        None
    );
}

#[test]
fn resource_memory_type_bits_are_respected() {
    let types = [memory_type(0, upload()), memory_type(1, upload())];
    assert_eq!(
        find_compatible_memory_type(&types, &requirements(2), upload(), |_| true),
        Some(1)
    );
}

#[test]
fn host_coherency_is_still_required() {
    let types = [
        memory_type(0, vk::MemoryPropertyFlags::HOST_VISIBLE),
        memory_type(1, upload()),
    ];
    assert_eq!(
        find_compatible_memory_type(&types, &requirements(3), upload(), |_| true),
        Some(1)
    );
}

#[test]
fn normal_selection_keeps_the_first_compatible_type() {
    let types = [memory_type(0, upload()), memory_type(1, upload())];
    assert_eq!(
        find_compatible_memory_type(&types, &requirements(3), upload(), |_| true),
        Some(0)
    );
}
