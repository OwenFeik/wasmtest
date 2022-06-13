use std::cell::RefCell;
use std::rc::Rc;

use js_sys::Array;
use parking_lot::Mutex;
use wasm_bindgen::prelude::*;

use crate::bridge::{
    expose_closure, expose_closure_array, expose_closure_f64, expose_closure_f64_bool,
    expose_closure_f64_f64, expose_closure_f64_string, expose_closure_string_in,
    expose_closure_string_out, layer_info, log, request_animation_frame,
};
use crate::client::Client;
use crate::viewport::Viewport;

fn logged_error<T>(error_message: &str) -> Result<T, JsValue> {
    log(error_message);
    Err(wasm_bindgen::JsValue::from_str(error_message))
}

#[wasm_bindgen(start)]
pub fn start() -> Result<(), JsValue> {
    let client = match Client::new() {
        Ok(c) => c,
        Err(_) => return logged_error("Failed to connect to game."),
    };

    let vp = match Viewport::new(client) {
        Ok(s) => Rc::new(Mutex::new(s)),
        Err(_) => return logged_error("Failed to create viewport."),
    };

    // This closure acquires the lock on the Viewport, then exports the scene
    // as a binary blob. This allows the front end to pull out the binary
    // representation of the scene to send back to the server.
    let vp_ref = vp.clone();
    let export_closure = Closure::wrap(Box::new(move || {
        let data = vp_ref.lock().scene.export();
        base64::encode(data)
    }) as Box<dyn FnMut() -> String>);
    expose_closure_string_out("export_scene", &export_closure);
    export_closure.forget();

    let vp_ref = vp.clone();
    let load_vp_closure = Closure::wrap(Box::new(move |vp_b64: String| {
        let s = match base64::decode(&vp_b64) {
            Ok(b) => match bincode::deserialize(&b) {
                Ok(s) => s,
                _ => return,
            },
            _ => return,
        };
        vp_ref.lock().scene.replace_scene(s);
    }) as Box<dyn FnMut(String)>);
    expose_closure_string_in("load_scene", &load_vp_closure);
    load_vp_closure.forget();

    let vp_ref = vp.clone();
    let new_vp_closure = Closure::wrap(Box::new(move |id: f64| {
        vp_ref.lock().scene.new_scene(id as i64);
    }) as Box<dyn FnMut(f64)>);
    expose_closure_f64("new_scene", &new_vp_closure);
    new_vp_closure.forget();

    let vp_ref = vp.clone();
    let new_sprite_closure = Closure::wrap(Box::new(move |id: f64, layer: f64| {
        vp_ref.lock().scene.new_sprite(id as i64, layer as i64);
    }) as Box<dyn FnMut(f64, f64)>);
    expose_closure_f64_f64("new_sprite", &new_sprite_closure);
    new_sprite_closure.forget();

    let vp_ref = vp.clone();
    let rename_layer_closure = Closure::wrap(Box::new(move |id: f64, title: String| {
        vp_ref.lock().scene.rename_layer(id as i64, title);
    }) as Box<dyn FnMut(f64, String)>);
    expose_closure_f64_string("rename_layer", &rename_layer_closure);
    rename_layer_closure.forget();

    let vp_ref = vp.clone();
    let layer_visibility_closure = Closure::wrap(Box::new(move |id: f64, visible: bool| {
        vp_ref.lock().scene.set_layer_visible(id as i64, visible);
    }) as Box<dyn FnMut(f64, bool)>);
    expose_closure_f64_bool("layer_visible", &layer_visibility_closure);
    layer_visibility_closure.forget();

    let vp_ref = vp.clone();
    let layer_locked_closure = Closure::wrap(Box::new(move |id: f64, locked: bool| {
        vp_ref.lock().scene.set_layer_locked(id as i64, locked);
    }) as Box<dyn FnMut(f64, bool)>);
    expose_closure_f64_bool("layer_locked", &layer_locked_closure);
    layer_locked_closure.forget();

    let vp_ref = vp.clone();
    let vp_layers_closure = Closure::wrap(
        Box::new(move || layer_info(vp_ref.lock().scene.layers())) as Box<dyn FnMut() -> Array>,
    );
    expose_closure_array("vp_layers", &vp_layers_closure);
    vp_layers_closure.forget();

    let vp_ref = vp.clone();
    let new_layer_closure = Closure::wrap(Box::new(move || {
        vp_ref.lock().scene.new_layer();
    }) as Box<dyn FnMut()>);
    expose_closure("new_layer", &new_layer_closure);
    new_layer_closure.forget();

    let vp_ref = vp.clone();
    let remove_layer_closure = Closure::wrap(Box::new(move |id: f64| {
        vp_ref.lock().scene.remove_layer(id as i64);
    }) as Box<dyn FnMut(f64)>);
    expose_closure_f64("remove_layer", &remove_layer_closure);
    remove_layer_closure.forget();

    let vp_ref = vp.clone();
    let move_layer_closure = Closure::wrap(Box::new(move |id: f64, up: bool| {
        vp_ref.lock().scene.move_layer(id as i64, up);
    }) as Box<dyn FnMut(f64, bool)>);
    expose_closure_f64_bool("move_layer", &move_layer_closure);
    move_layer_closure.forget();

    let f = Rc::new(RefCell::new(None));
    let g = f.clone();

    *g.borrow_mut() = Some(Closure::wrap(Box::new(move || {
        vp.lock().animation_frame();
        request_animation_frame(f.borrow().as_ref().unwrap()).unwrap();
    }) as Box<dyn FnMut()>));

    request_animation_frame(g.borrow().as_ref().unwrap()).unwrap();

    Ok(())
}
