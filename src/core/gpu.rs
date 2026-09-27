//! Asks the Vulkan driver what the GPU supports, to decide whether DXVK is a
//! safe recommendation. DXVK 3.x needs a Vulkan 1.4 driver, and its authors
//! advise AMD RDNA1/RDNA2 (RX 5000/6000) Windows users to stay on DXVK 2.x.

use std::sync::OnceLock;

#[derive(Clone, Debug)]
pub struct Gpu {
    pub name: String,
    pub api_major: u32,
    pub api_minor: u32,
    pub vendor_id: u32,
}

const AMD: u32 = 0x1002;

/// The best Vulkan device (discrete preferred, then newest API), probed once.
/// `None` when there's no Vulkan driver at all.
pub fn best() -> Option<&'static Gpu> {
    static GPU: OnceLock<Option<Gpu>> = OnceLock::new();
    GPU.get_or_init(probe).as_ref()
}

fn probe() -> Option<Gpu> {
    use ash::vk;
    // SAFETY: standard instance create/enumerate/destroy sequence; the entry
    // (and its loaded library) outlives the instance.
    unsafe {
        let entry = ash::Entry::load().ok()?;
        let app = vk::ApplicationInfo::default()
            .application_name(c"Vault Patcher")
            .api_version(vk::make_api_version(0, 1, 1, 0));
        let info = vk::InstanceCreateInfo::default().application_info(&app);
        let instance = entry.create_instance(&info, None).ok()?;
        let best = instance
            .enumerate_physical_devices()
            .unwrap_or_default()
            .into_iter()
            .map(|d| instance.get_physical_device_properties(d))
            .max_by_key(|p| (p.device_type == vk::PhysicalDeviceType::DISCRETE_GPU, p.api_version))
            .map(|p| Gpu {
                name: p
                    .device_name_as_c_str()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                api_major: vk::api_version_major(p.api_version),
                api_minor: vk::api_version_minor(p.api_version),
                vendor_id: p.vendor_id,
            });
        instance.destroy_instance(None);
        best
    }
}

/// `Ok` when DXVK 3 should run well, otherwise a short reason for the user.
pub fn dxvk_ready() -> Result<(), String> {
    let Some(gpu) = best() else {
        return Err("No Vulkan driver found. Update your graphics driver to use DXVK".into());
    };
    if (gpu.api_major, gpu.api_minor) < (1, 4) {
        return Err(format!(
            "{} reports Vulkan {}.{}; DXVK needs 1.4. Update your graphics driver",
            gpu.name, gpu.api_major, gpu.api_minor
        ));
    }
    let name = gpu.name.to_ascii_uppercase();
    if gpu.vendor_id == AMD && (name.contains("RX 5") || name.contains("RX 6")) {
        return Err(format!("{}: DXVK's authors recommend staying on the game's own renderer (DXVK 2.x) for this GPU", gpu.name));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    #[ignore]
    fn live_print_gpu() {
        println!("{:?}", super::best());
        println!("{:?}", super::dxvk_ready());
    }
}
