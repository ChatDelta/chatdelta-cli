//! Output formatting for ChatDelta CLI

use crate::cli::Args;
use std::fs::File;
use std::io::Write;

/// Output results in the specified format
pub fn output_results(
    args: &Args,
    responses: &[(String, String)],
    digest: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    match args.format.as_str() {
        "json" => output_json(args, responses, digest),
        "markdown" => output_markdown(args, responses, digest),
        _ => output_text(args, responses, digest),
    }
}

/// Output in JSON format
fn output_json(
    args: &Args,
    responses: &[(String, String)],
    digest: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut json_output = serde_json::Map::new();
    json_output.insert(
        "prompt".to_string(),
        serde_json::Value::String(args.prompt.as_ref().unwrap().clone()),
    );

    let mut responses_obj = serde_json::Map::new();
    for (name, response) in responses {
        responses_obj.insert(name.clone(), serde_json::Value::String(response.clone()));
    }
    json_output.insert(
        "responses".to_string(),
        serde_json::Value::Object(responses_obj),
    );

    if let Some(summary) = digest {
        json_output.insert(
            "summary".to_string(),
            serde_json::Value::String(summary.to_string()),
        );
    }

    println!("{}", serde_json::to_string_pretty(&json_output)?);
    Ok(())
}

/// Output in Markdown format
fn output_markdown(
    args: &Args,
    responses: &[(String, String)],
    digest: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("# ChatDelta Results\n");
    println!("**Prompt:** {}\n", args.prompt.as_ref().unwrap());

    for (name, response) in responses {
        println!("## {}\n", name);
        println!("{}\n", response);
    }

    if let Some(summary) = digest {
        println!("## Summary\n");
        println!("{}\n", summary);
    }

    Ok(())
}

/// Output in plain text format
fn output_text(
    args: &Args,
    responses: &[(String, String)],
    digest: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    if responses.len() == 1 {
        // Single response, just print it
        println!("{}", responses[0].1);
    } else {
        // Multiple responses, show them separately
        for (name, response) in responses {
            if args.verbose {
                println!("=== {} ===", name);
                println!("{}\n", response);
            }
        }

        if let Some(summary) = digest {
            if !args.verbose {
                println!("{}", summary);
            } else {
                println!("=== Summary ===");
                println!("{}", summary);
            }
        } else if !args.verbose {
            // No summary, show the first response
            println!("{}", responses[0].1);
        }
    }

    Ok(())
}

/// Log the interaction to a file
pub fn log_interaction(
    args: &Args,
    responses: &[(String, String)],
    digest: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(path) = &args.log {
        match File::create(path) {
            Ok(mut file) => {
                let prompt = args.prompt.as_ref().unwrap();
                let _ = writeln!(file, "Prompt:\n{}\n", prompt);
                for (name, response) in responses {
                    let _ = writeln!(file, "{}:\n{}\n", name, response);
                }
                if let Some(summary) = digest {
                    let _ = writeln!(file, "Summary:\n{}\n", summary);
                }
                if !args.quiet {
                    println!("✓ Conversation logged to {}", path.display());
                }
            }
            Err(e) => {
                if !args.quiet {
                    eprintln!("Warning: Failed to create log file {}: {}", path.display(), e);
                }
            }
        }
    }
    Ok(())
}