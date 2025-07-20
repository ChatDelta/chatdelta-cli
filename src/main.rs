//! ChatDelta CLI application
//!
//! A command-line tool for querying multiple AI APIs and summarizing their responses.

use chatdelta::{
    AiClient, ClientConfig, create_client, execute_parallel, generate_summary
};
use clap::Parser;
use std::env;
use std::time::Duration;

mod cli;
mod output;
mod logging;

use cli::Args;
use output::{output_results, log_interaction};
use logging::Logger;

/// Main application logic
async fn run(args: Args) -> Result<(), Box<dyn std::error::Error>> {
    // Validate arguments first
    args.validate()?;

    // Handle special commands
    if args.list_models {
        print_available_models();
        return Ok(());
    }

    if args.test {
        if !args.quiet {
            println!("Testing API connections...");
        }
        return test_connections(&args).await;
    }

    // Create client configuration using the builder pattern from chatdelta 0.2.0
    let mut config_builder = ClientConfig::builder()
        .timeout(Duration::from_secs(args.timeout))
        .retries(args.retries)
        .max_tokens(args.max_tokens);
    
    if let Some(temp) = args.temperature {
        config_builder = config_builder.temperature(temp);
    }
    
    let config = config_builder.build();

    // Create AI clients based on available API keys and user selection
    let mut clients: Vec<Box<dyn AiClient>> = Vec::new();
    
    if args.should_use_ai("gpt") {
        if let Ok(key) = env::var("OPENAI_API_KEY") {
            match create_client("openai", &key, &args.gpt_model, config.clone()) {
                Ok(client) => clients.push(client),
                Err(e) => {
                    if !args.quiet {
                        eprintln!("Warning: Failed to create ChatGPT client: {}", e);
                    }
                }
            }
        } else if !args.quiet {
            eprintln!("Warning: OPENAI_API_KEY not set, skipping ChatGPT");
        }
    }
    
    if args.should_use_ai("gemini") {
        if let Ok(key) = env::var("GEMINI_API_KEY") {
            match create_client("gemini", &key, &args.gemini_model, config.clone()) {
                Ok(client) => clients.push(client),
                Err(e) => {
                    if !args.quiet {
                        eprintln!("Warning: Failed to create Gemini client: {}", e);
                    }
                }
            }
        } else if !args.quiet {
            eprintln!("Warning: GEMINI_API_KEY not set, skipping Gemini");
        }
    }
    
    if args.should_use_ai("claude") {
        if let Ok(key) = env::var("ANTHROPIC_API_KEY") {
            match create_client("claude", &key, &args.claude_model, config.clone()) {
                Ok(client) => clients.push(client),
                Err(e) => {
                    if !args.quiet {
                        eprintln!("Warning: Failed to create Claude client: {}", e);
                    }
                }
            }
        } else if !args.quiet {
            eprintln!("Warning: ANTHROPIC_API_KEY not set, skipping Claude");
        }
    }

    if clients.is_empty() {
        return Err("No AI clients available. Check your API keys and --only/--exclude settings.".into());
    }

    // Initialize comprehensive logger
    let mut logger = if args.log_metrics || args.log_errors || args.log_dir.is_some() {
        Some(Logger::new(&args)?)
    } else {
        None
    };

    // Query each model with the same prompt in parallel
    if !args.quiet {
        println!("Querying {} AI model{}...", clients.len(), if clients.len() == 1 { "" } else { "s" });
    }
    
    let prompt = args.prompt.as_ref().unwrap();
    
    // Start logging interaction
    if let Some(ref mut logger) = logger {
        logger.start_interaction(prompt);
    }
    
    let query_start = std::time::Instant::now();
    let results = execute_parallel(clients, prompt).await;
    let query_duration = query_start.elapsed();
    
    let mut responses = Vec::new();
    
    for (name, result) in results {
        match result {
            Ok(reply) => {
                if args.verbose {
                    println!("✓ Received response from {}", name);
                }
                
                // Log successful response
                if let Some(ref mut logger) = logger {
                    logger.log_model_response(&name, Ok(&reply), query_duration, None);
                }
                
                responses.push((name, reply));
            }
            Err(e) => {
                if !args.quiet {
                    eprintln!("✗ {} error: {}", name, e);
                }
                
                // Log error
                if let Some(ref mut logger) = logger {
                    logger.log_model_response(&name, Err(&e.to_string()), query_duration, None);
                    logger.log_error(&name, "API_ERROR", &e.to_string(), None);
                }
            }
        }
    }
    
    if responses.is_empty() {
        return Err("No successful responses from any AI models".into());
    }
    
    if !args.quiet {
        println!("✓ Received {} response{}", responses.len(), if responses.len() == 1 { "" } else { "s" });
    }

    // Generate summary if requested and we have multiple responses
    let (digest, summary_duration) = if !args.no_summary && responses.len() > 1 {
        if !args.quiet {
            println!("Generating summary...");
        }
        
        let summary_start = std::time::Instant::now();
        
        // Use Gemini for summary if available, otherwise use the first client
        let summary_client = if let Ok(key) = env::var("GEMINI_API_KEY") {
            create_client("gemini", &key, &args.gemini_model, config).ok()
        } else {
            None
        };
        
        if let Some(client) = summary_client {
            match generate_summary(&*client, &responses).await {
                Ok(summary) => {
                    let duration = summary_start.elapsed();
                    if !args.quiet {
                        println!("✓ Summary generated");
                    }
                    
                    // Log summary
                    if let Some(ref mut logger) = logger {
                        logger.set_summary(&summary);
                    }
                    
                    (Some(summary), Some(duration))
                }
                Err(e) => {
                    if !args.quiet {
                        eprintln!("Warning: Summary generation failed: {}", e);
                    }
                    
                    // Log summary error
                    if let Some(ref mut logger) = logger {
                        logger.log_error("summary", "GENERATION_ERROR", &e.to_string(), None);
                    }
                    
                    (None, None)
                }
            }
        } else {
            (None, None)
        }
    } else {
        (None, None)
    };

    // Output results
    output_results(&args, &responses, digest.as_deref())?;
    
    // Log interaction if requested (legacy simple logging)
    log_interaction(&args, &responses, digest.as_deref())?;
    
    // Finalize comprehensive logging
    if let Some(mut logger) = logger {
        logger.finalize_interaction(summary_duration)?;
        
        if !args.quiet && (args.log_metrics || args.log_errors || args.log_dir.is_some()) {
            if let Ok(stats) = logger.get_log_stats() {
                println!("✓ Logged to structured logs ({} files, {})", 
                    stats.total_files, stats.size_human_readable());
            }
        }
    }

    Ok(())
}

/// Print available models
fn print_available_models() {
    println!("Available models:");
    println!("  OpenAI: gpt-4o, gpt-4o-mini, gpt-4-turbo, gpt-3.5-turbo");
    println!("  Gemini: gemini-1.5-pro-latest, gemini-1.5-flash-latest, gemini-pro");
    println!("  Claude: claude-3-5-sonnet-20241022, claude-3-haiku-20240307, claude-3-opus-20240229");
}

/// Test API connections
async fn test_connections(args: &Args) -> Result<(), Box<dyn std::error::Error>> {
    let mut config_builder = ClientConfig::builder()
        .timeout(Duration::from_secs(args.timeout))
        .retries(0) // No retries for tests
        .max_tokens(args.max_tokens);
    
    if let Some(temp) = args.temperature {
        config_builder = config_builder.temperature(temp);
    }
    
    let config = config_builder.build();
    
    let test_prompt = "Hello, please respond with just 'OK' to confirm you're working.";
    let mut all_passed = true;
    
    if args.should_use_ai("gpt") {
        match env::var("OPENAI_API_KEY") {
            Ok(key) => {
                match create_client("openai", &key, &args.gpt_model, config.clone()) {
                    Ok(client) => {
                        match client.send_prompt(test_prompt).await {
                            Ok(_) => println!("✓ ChatGPT connection successful"),
                            Err(e) => {
                                println!("✗ ChatGPT connection failed: {}", e);
                                all_passed = false;
                            }
                        }
                    }
                    Err(e) => {
                        println!("✗ ChatGPT client creation failed: {}", e);
                        all_passed = false;
                    }
                }
            }
            Err(_) => {
                println!("✗ ChatGPT: OPENAI_API_KEY not set");
                all_passed = false;
            }
        }
    }
    
    if args.should_use_ai("gemini") {
        match env::var("GEMINI_API_KEY") {
            Ok(key) => {
                match create_client("gemini", &key, &args.gemini_model, config.clone()) {
                    Ok(client) => {
                        match client.send_prompt(test_prompt).await {
                            Ok(_) => println!("✓ Gemini connection successful"),
                            Err(e) => {
                                println!("✗ Gemini connection failed: {}", e);
                                all_passed = false;
                            }
                        }
                    }
                    Err(e) => {
                        println!("✗ Gemini client creation failed: {}", e);
                        all_passed = false;
                    }
                }
            }
            Err(_) => {
                println!("✗ Gemini: GEMINI_API_KEY not set");
                all_passed = false;
            }
        }
    }
    
    if args.should_use_ai("claude") {
        match env::var("ANTHROPIC_API_KEY") {
            Ok(key) => {
                match create_client("claude", &key, &args.claude_model, config.clone()) {
                    Ok(client) => {
                        match client.send_prompt(test_prompt).await {
                            Ok(_) => println!("✓ Claude connection successful"),
                            Err(e) => {
                                println!("✗ Claude connection failed: {}", e);
                                all_passed = false;
                            }
                        }
                    }
                    Err(e) => {
                        println!("✗ Claude client creation failed: {}", e);
                        all_passed = false;
                    }
                }
            }
            Err(_) => {
                println!("✗ Claude: ANTHROPIC_API_KEY not set");
                all_passed = false;
            }
        }
    }
    
    if all_passed {
        println!("\n✓ All API connections working properly");
        Ok(())
    } else {
        Err("Some API connections failed".into())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    if let Err(e) = run(args).await {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::Args;
    use std::env;

    #[test]
    fn test_args_parsing() {
        let args = Args::try_parse_from(["chatdelta", "Hello, world!"])
            .expect("Failed to parse basic arguments");
        assert_eq!(args.prompt, Some("Hello, world!".to_string()));
        assert!(args.log.is_none());
        args.validate()
            .expect("Valid arguments should pass validation");
    }

    #[test]
    fn test_args_validate_empty() {
        let args = Args::try_parse_from(["chatdelta", ""])
            .expect("Should parse test arguments");
        assert!(args.validate().is_err());
    }

    #[tokio::test]
    async fn test_run_missing_env_vars() {
        unsafe {
            env::remove_var("OPENAI_API_KEY");
            env::remove_var("GEMINI_API_KEY");
            env::remove_var("ANTHROPIC_API_KEY");
        }
        let args = Args::try_parse_from(["chatdelta", "Test"])
            .expect("Should parse test arguments");
        let result = run(args).await;
        assert!(result.is_err());
    }
}