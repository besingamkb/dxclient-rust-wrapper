use std::{env, fs};
use std::io::{BufReader, BufRead};
use std::path::Path;
use std::process::{Command, Stdio};
use atty::Stream;

fn main() {
    let image_name = "dxclient";
    let image_tag = "local";
    let os_type = env::consts::OS;
    let is_windows = os_type == "windows";
    println!("Is Windows? {}", is_windows);

    // parse command-line arguments
    let args: Vec<String> = env::args().skip(1).collect();

    let container_folder = "/dxclient/store";
    let args: Vec<String> = args
        .iter()
        .map(|arg| {
            if Path::new(arg).exists() {
                let base_name = Path::new(arg).file_name().unwrap().to_string_lossy();
                return match is_windows {
                    false => format!("{}/{}", container_folder, base_name),
                    true => format!("{}\\{}", container_folder, base_name),
                }
            }
            arg.to_string()
        }).collect();

    //check for dependencies
    // basically check if docker is registered properly on PATH
    if let Err(err) = check_dependencies("docker") {
        println!("{}", err);
        return;
    }

    // handle environment variables
    let volume_dir = env::var("VOLUME_DIR").unwrap_or_else(|_| "store".to_string());
    let container_runtime = env::var("CONTAINER_RUNTIME").unwrap_or_else(|_| "docker".to_string());

    // determine if running in a TTY
    let tty_flag = match is_tty() {
        true => "-t",
        false => "",
    };

    // create volume directory if they don't exist
    if let Err(err) = fs::create_dir_all(&volume_dir) {
        println!("Error creating volume directory: {}", err);
        return;
    }

    // generate volume parameters
    let volume_params = format!(
        "-v \"{}/{}:/dxclient/store\":Z",
        env::current_dir().unwrap().to_string_lossy(),
        volume_dir
    );

    // compose docker command
    let docker_cmd = format!(
        "{} run -e VOLUME_DIR=\"{}\" {} {} --network=host --platform linux/amd64 --name dxclient --rm {}:{} ./bin/dxclient {}",
        container_runtime, volume_dir, tty_flag, volume_params, image_name, image_tag, args.join(" ")
    );

    let mut biding = match is_windows {
        true => Command::new("cmd"),
        false => Command::new("sh"),
    };

    let child = match is_windows {
        true => biding.arg("/C"),
        false => biding.arg("-c"),
    };
    println!("generated docker command: {}", docker_cmd);
    child.arg(docker_cmd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child_process = child.spawn().expect("Failed to execute command");

    // read and print stdout
    let stdout = child_process.stdout.take().unwrap();
    let stdout_reader = BufReader::new(stdout);
    for line in stdout_reader.lines() {
        println!("{}", line.expect("Failed to read STDOUT"));
    }

    // read and print stderr
    let stderr = child_process.stderr.take().unwrap();
    let stderr_reader = BufReader::new(stderr);
    for line in stderr_reader.lines() {
        println!("{}", line.expect("Failed to read STDERR"));
    }

    // wait for command to finish
    let status = child_process.wait().expect("Failed to wait for command");
    if !status.success() {
        println!("Error executing docker command: {:?}", status);
        return;
    }

    cleanup_files(&args, &volume_dir);
}
fn check_dependencies(cmd: &str) -> Result<(), String> {
    match Command::new("sh").arg("-c").arg(format!("{} -v", cmd)).output() {
        Ok(output) => {
            if output.status.success() {
                Ok(())
            } else {
                Err(format!("{} command not found", cmd))
            }
        }
        Err(err) => Err(format!("Error checking dependencies: {}", err)),
    }
}

fn is_tty() -> bool {
    atty::is(Stream::Stdout)
}

fn cleanup_files(args: &[String], volume_dir: &str) {
    for arg in args {
        if Path::new(arg).exists() {
            let abs_path = Path::new(volume_dir).join(Path::new(arg).file_name().unwrap());
            if let Err(err) = fs::remove_file(&abs_path) {
                println!("Error removing file {}: {}", abs_path.display(), err);
            }
        }
    }
}