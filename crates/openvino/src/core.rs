//! Define the core interface between Rust and OpenVINO's C
//! [API](https://docs.openvinotoolkit.org/latest/ie_c_api/modules.html).

use crate::blob::Blob;
use crate::tensor_desc::TensorDesc;
use crate::{cstr, drop_using_function, try_unsafe, util::Result};
use crate::{
    error::{LoadingError, SetupError},
    network::{CNNNetwork, ExecutableNetwork},
};
use crate::{Layout, Precision};
use openvino_sys::{
    self, ie_config_t, ie_core_create, ie_core_free, ie_core_load_network, ie_core_read_network,
    ie_core_read_network_from_memory, ie_core_t,
};

const NUM_THREADS: i32 = 1;

/// See [Core](https://docs.openvinotoolkit.org/latest/classInferenceEngine_1_1Core.html).
pub struct Core {
    instance: *mut ie_core_t,
}
drop_using_function!(Core, ie_core_free);

unsafe impl Send for Core {}

impl Core {
    /// Construct a new OpenVINO [`Core`]--this is the primary entrypoint for constructing and using
    /// inference networks. Because this function may load OpenVINO's shared libraries at runtime,
    /// there are more ways than usual that this function can fail (e.g., [`LoadingError`]s).
    pub fn new(xml_config_file: Option<&str>) -> std::result::Result<Core, SetupError> {
        openvino_sys::library::load().map_err(LoadingError::SystemFailure)?;

        let file = if let Some(file) = xml_config_file {
            cstr!(file.to_string())
        } else if let Some(file) = openvino_finder::find_plugins_xml() {
            cstr!(file
                .to_str()
                .ok_or(LoadingError::CannotStringifyPath)?
                .to_string())
        } else {
            cstr!(String::new())
        };

        let mut instance = std::ptr::null_mut();
        try_unsafe!(ie_core_create(file, std::ptr::addr_of_mut!(instance)))?;
        Ok(Core { instance })
    }

    /// Read a [`CNNNetwork`] from a pair of files: `model_path` points to an XML file containing the
    /// OpenVINO network IR and `weights_path` points to the binary weights file.
    pub fn read_network_from_file(
        &mut self,
        model_path: &str,
        weights_path: &str,
    ) -> Result<CNNNetwork> {
        let mut instance = std::ptr::null_mut();
        try_unsafe!(ie_core_read_network(
            self.instance,
            cstr!(model_path),
            cstr!(weights_path),
            std::ptr::addr_of_mut!(instance)
        ))?;
        Ok(CNNNetwork { instance })
    }

    /// Read a [`CNNNetwork`] from a pair of byte slices: `model_content` contains the XML data
    /// describing the OpenVINO network IR and `weights_content` contains the binary weights.
    pub fn read_network_from_buffer(
        &mut self,
        model_content: &[u8],
        weights_content: &[u8],
    ) -> Result<CNNNetwork> {
        let mut instance = std::ptr::null_mut();
        let weights_desc = TensorDesc::new(Layout::ANY, &[weights_content.len()], Precision::U8);
        let weights_blob = Blob::new(&weights_desc, weights_content)?;
        try_unsafe!(ie_core_read_network_from_memory(
            self.instance,
            model_content.as_ptr().cast::<u8>(),
            model_content.len(),
            weights_blob.instance,
            std::ptr::addr_of_mut!(instance)
        ))?;
        Ok(CNNNetwork { instance })
    }

    /// Instantiate a [`CNNNetwork`] as an [`ExecutableNetwork`] on the specified `device`.
    pub fn load_network(
        &mut self,
        network: &CNNNetwork,
        device: &str,
    ) -> Result<ExecutableNetwork> {
        let mut instance = std::ptr::null_mut();
        // Because `ie_core_load_network` does not allow a null pointer for the configuration, we
        // construct an empty configuration struct to pass. At some point, it could be good to allow
        // users to pass a map to this function that gets converted to an `ie_config_t` (TODO).
        let empty_config = ie_config_t {
            name: cstr!("INFERENCE_NUM_THREADS"),
            value: std::ptr::addr_of!(NUM_THREADS),
            next: std::ptr::null_mut(),
        };

        try_unsafe!(ie_core_load_network(
            self.instance,
            network.instance,
            cstr!(device),
            std::ptr::addr_of!(empty_config),
            std::ptr::addr_of_mut!(instance)
        ))?;
        Ok(ExecutableNetwork { instance })
    }
}
