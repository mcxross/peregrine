use crate::sui::project::{bytecode_targets, resolve_context};
use crate::workbench::prelude::*;
use std::ffi::OsStr;
use std::path::Path;
use std::sync::mpsc;
use std::thread;

impl App {
    pub(crate) fn ensure_bytecode_session(&mut self) {
        if matches!(&self.bytecode, BytecodePane::Loading(_)) {
            self.status = String::from("Bytecode is already loading");
            return;
        }

        let options = match self.current_bytecode_options() {
            Ok(options) => options,
            Err(message) => {
                self.bytecode.set_message(message);
                return;
            }
        };

        if self.bytecode.ready_matches_any(&options)
            || self.bytecode.selector_matches(&options)
            || self.bytecode.loading_matches_any(&options)
        {
            return;
        }

        match options.targets.as_slice() {
            [] => self
                .bytecode
                .set_message("No Move module matched the requested bytecode target.".to_string()),
            [target] => {
                let request = BytecodeRequest::new(options.context, target.clone());
                self.load_bytecode_request(request);
            }
            _ => {
                self.status = format!("Select a module from {}", options.package_name);
                self.bytecode = BytecodePane::Selecting(BytecodeSelector::new(options));
            }
        }
    }

    pub(crate) fn load_bytecode_request(&mut self, request: BytecodeRequest) {
        let stamp = bytecode_cache_stamp(&request.context.package_root);
        if let Some(entry) = self.bytecode_cache.get(&request.key).cloned() {
            if entry.stamp == stamp {
                self.status = format!(
                    "Loaded bytecode for {}::{} from cache",
                    entry.session.package_name, entry.session.key.module_name
                );
                self.bytecode = BytecodePane::Ready(entry.session);
                return;
            }
            self.bytecode_cache.remove(&request.key);
        }

        if self.bytecode.is_loading_key(&request.key) {
            self.status = String::from("Bytecode is already loading");
            return;
        }

        self.bytecode_load_epoch = self.bytecode_load_epoch.wrapping_add(1);
        let epoch = self.bytecode_load_epoch;
        let key = request.key.clone();
        let result_key = key.clone();
        let package_name = request.package_name.clone();
        let module_name = request.key.module_name.clone();
        let (tx, rx) = mpsc::channel();

        match thread::Builder::new()
            .name(format!("peregrine-sui-bytecode-{module_name}"))
            .spawn(move || {
                let result = BytecodeSession::load(request);
                let _ = tx.send(BytecodeLoadResult {
                    epoch,
                    key: result_key,
                    stamp,
                    result,
                });
            }) {
            Ok(_) => {
                self.bytecode_loader_rx = Some(rx);
                self.status = format!("Loading bytecode for {package_name}::{module_name}");
                self.bytecode = BytecodePane::Loading(BytecodeLoadState {
                    key,
                    package_name,
                    module_name,
                    stamp,
                    epoch,
                });
            }
            Err(error) => {
                let message = format!("Could not start bytecode loader: {error}");
                self.status = format!("Bytecode failed: {message}");
                self.bytecode = BytecodePane::Message(message);
            }
        }
    }

    pub(crate) fn drain_bytecode_loader(&mut self) {
        let event = match self.bytecode_loader_rx.as_ref() {
            Some(rx) => match rx.try_recv() {
                Ok(result) => Some(Ok(result)),
                Err(mpsc::TryRecvError::Empty) => None,
                Err(mpsc::TryRecvError::Disconnected) => Some(Err(
                    "Bytecode loader stopped before returning a result.".to_string(),
                )),
            },
            None => None,
        };

        match event {
            Some(Ok(result)) => {
                self.bytecode_loader_rx = None;
                self.apply_bytecode_load_result(result);
            }
            Some(Err(message)) => {
                self.bytecode_loader_rx = None;
                if matches!(&self.bytecode, BytecodePane::Loading(_)) {
                    self.status = format!("Bytecode failed: {message}");
                    self.bytecode = BytecodePane::Message(message);
                }
            }
            None => {}
        }
    }

    pub(crate) fn apply_bytecode_load_result(&mut self, result: BytecodeLoadResult) {
        let is_current = matches!(
            &self.bytecode,
            BytecodePane::Loading(load)
                if load.epoch == result.epoch
                    && load.key == result.key
                    && load.stamp == result.stamp
        );

        if !is_current {
            return;
        }

        if bytecode_cache_stamp(&result.key.package_root) != result.stamp {
            self.status =
                String::from("Package changed while bytecode was loading; press Enter to reload");
            self.bytecode = BytecodePane::Empty;
            return;
        }

        match result.result {
            Ok(session) => {
                self.status = format!(
                    "Loaded bytecode for {}::{}",
                    session.package_name, session.key.module_name
                );
                self.bytecode_cache.insert(
                    result.key.clone(),
                    BytecodeCacheEntry {
                        stamp: result.stamp,
                        session: session.clone(),
                    },
                );
                self.bytecode = BytecodePane::Ready(session);
            }
            Err(message) => {
                self.status = format!("Bytecode failed: {message}");
                self.bytecode = BytecodePane::Message(message);
            }
        }
    }

    pub(crate) fn show_bytecode_selector(&mut self) {
        match self.current_bytecode_options() {
            Ok(options) if options.targets.is_empty() => self
                .bytecode
                .set_message("No Move module matched the requested bytecode target.".to_string()),
            Ok(options) => {
                self.status = format!("Select a module from {}", options.package_name);
                self.bytecode = BytecodePane::Selecting(BytecodeSelector::new(options));
            }
            Err(message) => self.bytecode.set_message(message),
        }
    }

    pub(crate) fn current_bytecode_options(&self) -> Result<BytecodeOptions, String> {
        if self.editor.dirty {
            return Err("Save the current file before opening the bytecode view.".to_string());
        }

        let project_root = self.explorer.root.clone();
        let source_hint = self
            .editor
            .path
            .as_ref()
            .cloned()
            .or_else(|| self.explorer.selected_path().map(Path::to_path_buf));
        let package_root = source_hint
            .as_deref()
            .and_then(|path| nearest_move_package_root(path, &project_root))
            .or_else(|| nearest_move_package_root(&project_root, &project_root))
            .ok_or_else(|| {
                "Open or select a Move source file inside a package with Move.toml.".to_string()
            })?;
        let package_path = relative_path_label(&project_root, &package_root);
        let context =
            resolve_context(&project_root, &package_path).map_err(|error| error.message)?;
        let file = source_hint
            .as_ref()
            .filter(|path| path.is_file() && path.extension() == Some(OsStr::new("move")))
            .and_then(|path| path.strip_prefix(&project_root).ok())
            .map(normalized_path_string);
        let targets =
            bytecode_targets(&context, None, file.as_deref()).map_err(|error| error.message)?;

        Ok(BytecodeOptions::new(context, file, targets))
    }
}
