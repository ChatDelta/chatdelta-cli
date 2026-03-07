//! ChatDelta CLI application
//!
//! A command-line tool for querying multiple AI APIs and summarizing their responses.

use chatdelta::{
    create_client, execute_parallel, execute_parallel_with_metadata, generate_summary, AiClient,
    ChatSession, ClientConfig, Message, RetryStrategy, StreamChunk,
};
use clap::Parser;
use std::env;
use std::fs;
use std::io::{self, Read};
use std::path::Path;
use std::time::Duration;
use tokio::sync::mpsc;

mod cli;
mod debate;
mod logging;
mod metrics_display;
mod output;

use cli::{Args, Commands, DebateArgs};
use logging::Logger;
use output::{log_interaction, output_results};

/// Main application logic
async fn run(mut args: Args) -> Result<(), Box<dyn std::error::Error>> {
    // Validate arguments first
    args.validate()?;

    // Handle reading prompt from stdin or file
    if args.prompt.as_deref() == Some("-") {
        let mut buffer = String::new();
        io::stdin().read_to_string(&mut buffer)?;
        args.prompt = Some(buffer.trim().to_string());
        if args.prompt.as_ref().map_or(true, |p| p.is_empty()) {
            return Err("No prompt provided via stdin".into());
        }
    } else if let Some(prompt_file) = &args.prompt_file {
        let content = fs::read_to_string(prompt_file)
            .map_err(|e| format!("Failed to read prompt file: {}", e))?;
        args.prompt = Some(content.trim().to_string());
        if args.prompt.as_ref().map_or(true, |p| p.is_empty()) {
            return Err("Prompt file is empty".into());
        }
    }

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

    if args.doctor {
        return run_doctor(&args);
    }

    // Handle conversation mode
    if args.conversation {
        return run_conversation_mode(&args).await;
    }

    // Create client configuration using the builder pattern
    let mut config_builder = ClientConfig::builder()
        .timeout(Duration::from_secs(args.timeout))
        .retries(args.retries)
        .max_tokens(args.max_tokens);

    if let Some(temp) = args.temperature {
        config_builder = config_builder.temperature(temp);
    }

    // Set retry strategy
    let retry_strategy = match args.retry_strategy.as_str() {
        "linear" => RetryStrategy::Linear(Duration::from_secs(1)),
        "fixed" => RetryStrategy::Fixed(Duration::from_secs(2)),
        _ => RetryStrategy::Exponential(Duration::from_secs(1)),
    };
    config_builder = config_builder.retry_strategy(retry_strategy);

    // Wire up system prompt (available since chatdelta 0.4.0)
    if let Some(ref system_prompt) = args.system_prompt {
        config_builder = config_builder.system_message(system_prompt);
    }

    let config = config_builder.build();

    // Create AI clients based on available API keys and user selection
    let mut clients: Vec<Box<dyn AiClient>> = Vec::new();

    if args.should_use_ai("gpt") {
        let openai_key = env::var("OPENAI_API_KEY")
            .or_else(|_| env::var("CHATGPT_API_KEY"));

        if let Ok(key) = openai_key {
            match create_client("openai", &key, &args.gpt_model, config.clone()) {
                Ok(client) => clients.push(client),
                Err(e) => {
                    if !args.quiet {
                        eprintln!("Warning: Failed to create ChatGPT client: {}", e);
                    }
                }
            }
        } else if !args.quiet {
            eprintln!("Warning: OPENAI_API_KEY or CHATGPT_API_KEY not set, skipping ChatGPT");
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
        let anthropic_key = env::var("ANTHROPIC_API_KEY")
            .or_else(|_| env::var("CLAUDE_API_KEY"));

        if let Ok(key) = anthropic_key {
            match create_client("claude", &key, &args.claude_model, config.clone()) {
                Ok(client) => clients.push(client),
                Err(e) => {
                    if !args.quiet {
                        eprintln!("Warning: Failed to create Claude client: {}", e);
                    }
                }
            }
        } else if !args.quiet {
            eprintln!("Warning: ANTHROPIC_API_KEY or CLAUDE_API_KEY not set, skipping Claude");
        }
    }

    if clients.is_empty() {
        return Err(
            "No AI clients available. Check your API keys and --only/--exclude settings.".into(),
        );
    }

    // Streaming path: single-model only, prints tokens as they arrive
    if args.stream {
        if clients.len() > 1 && !args.quiet {
            eprintln!(
                "Note: --stream requires a single model. Use --only to select one. \
                 Falling back to parallel mode."
            );
        } else if clients.len() == 1 {
            let client = clients.remove(0);
            let prompt = args.prompt.as_ref().ok_or("No prompt provided")?.clone();

            if args.show_usage {
                // Streaming doesn't return metadata; use send_prompt_with_metadata instead
                let name = client.name().to_string();
                let response = client.send_prompt_with_metadata(&prompt).await?;
                println!("{}", response.content);
                print_usage_table(&[(name, response.metadata.total_tokens, response.metadata.latency_ms)]);
            } else {
                let (tx, mut rx) = mpsc::unbounded_channel::<StreamChunk>();
                tokio::spawn(async move {
                    if let Err(e) = client.send_prompt_streaming(&prompt, tx).await {
                        eprintln!("Stream error: {}", e);
                    }
                });
                use std::io::Write;
                while let Some(chunk) = rx.recv().await {
                    print!("{}", chunk.content);
                    io::stdout().flush().ok();
                    if chunk.finished {
                        println!();
                        break;
                    }
                }
            }
            return Ok(());
        }
    }

    // Initialize comprehensive logger
    let mut logger = if args.log_metrics || args.log_errors || args.log_dir.is_some() {
        Some(Logger::new(&args)?)
    } else {
        None
    };

    // Query each model with the same prompt in parallel
    if !args.quiet && args.progress {
        println!(
            "\u{1f504} Querying {} AI model{}...",
            clients.len(),
            if clients.len() == 1 { "" } else { "s" }
        );
    } else if !args.quiet {
        println!(
            "Querying {} AI model{}...",
            clients.len(),
            if clients.len() == 1 { "" } else { "s" }
        );
    }

    let prompt = args.prompt.as_ref().ok_or("No prompt provided")?;

    // Show prompt preview if verbose
    if args.verbose && !args.quiet {
        let preview = if prompt.len() > 100 {
            format!("{}...", &prompt[..100])
        } else {
            prompt.clone()
        };
        println!("\u{1f4dd} Prompt: {}", preview);
    }

    // Start logging interaction
    if let Some(ref mut logger) = logger {
        logger.start_interaction(prompt);
    }

    let query_start = std::time::Instant::now();
    let (results, usage_rows) = if args.show_usage {
        let raw = execute_parallel_with_metadata(clients, prompt).await;
        let mut plain: Vec<(String, Result<String, _>)> = Vec::new();
        let mut usage: Vec<(String, Option<u32>, Option<u64>)> = Vec::new();
        for (name, result) in raw {
            match result {
                Ok(r) => {
                    usage.push((name.clone(), r.metadata.total_tokens, r.metadata.latency_ms));
                    plain.push((name, Ok(r.content)));
                }
                Err(e) => plain.push((name, Err(e))),
            }
        }
        (plain, usage)
    } else {
        (execute_parallel(clients, prompt).await, Vec::new())
    };
    let query_duration = query_start.elapsed();

    let mut responses = Vec::new();

    for (name, result) in results {
        match result {
            Ok(reply) => {
                if args.verbose {
                    println!("\u{2705} Received response from {} ({} chars)", name, reply.len());
                }

                // Save individual response if requested
                if let Some(dir) = &args.save_responses {
                    save_individual_response(dir, &name, &reply)?;
                }

                // Log successful response
                if let Some(ref mut logger) = logger {
                    logger.log_model_response(&name, Ok(&reply), query_duration, None);
                }

                responses.push((name, reply));
            }
            Err(e) => {
                if !args.quiet {
                    // Provide actionable error messages based on common patterns
                    let msg = e.to_string();
                    if msg.contains("401") || msg.contains("Unauthorized") || msg.contains("invalid_api_key") {
                        eprintln!("\u{2717} {} error: Invalid API key — check your environment variables", name);
                    } else if msg.contains("429") || msg.contains("rate") || msg.contains("RateLimit") {
                        eprintln!("\u{2717} {} error: Rate limit exceeded — retry after a moment", name);
                    } else if msg.contains("timeout") || msg.contains("Timeout") {
                        eprintln!("\u{2717} {} error: Request timed out — try --timeout with a higher value", name);
                    } else {
                        eprintln!("\u{2717} {} error: {}", name, e);
                    }
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
        println!(
            "\u{2713} Received {} response{}",
            responses.len(),
            if responses.len() == 1 { "" } else { "s" }
        );
    }

    // Generate summary if requested and we have multiple responses
    let (digest, summary_duration) = if !args.no_summary && responses.len() > 1 {
        if !args.quiet {
            println!("Generating summary...");
        }

        let summary_start = std::time::Instant::now();

        // Try to use Gemini for summary, fall back to Claude, then OpenAI
        let summary_client = if let Ok(key) = env::var("GEMINI_API_KEY") {
            create_client("gemini", &key, &args.gemini_model, config.clone()).ok()
        } else if let Ok(key) = env::var("ANTHROPIC_API_KEY")
            .or_else(|_| env::var("CLAUDE_API_KEY")) {
            create_client("claude", &key, &args.claude_model, config.clone()).ok()
        } else if let Ok(key) = env::var("OPENAI_API_KEY")
            .or_else(|_| env::var("CHATGPT_API_KEY")) {
            create_client("openai", &key, &args.gpt_model, config).ok()
        } else {
            None
        };

        if let Some(client) = summary_client {
            match generate_summary(&*client, &responses).await {
                Ok(summary) => {
                    let duration = summary_start.elapsed();
                    if !args.quiet {
                        println!("\u{2713} Summary generated");
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
    if args.raw {
        // Raw output mode - just print responses
        for (_, response) in &responses {
            println!("{}", response);
        }
    } else {
        output_results(&args, &responses, digest.as_deref())?;
    }

    // Show token usage table if requested
    if !usage_rows.is_empty() {
        print_usage_table(&usage_rows);
    }

    // Log interaction if requested (legacy simple logging)
    log_interaction(&args, &responses, digest.as_deref())?;

    // Finalize comprehensive logging
    if let Some(mut logger) = logger {
        logger.finalize_interaction(summary_duration)?;

        if !args.quiet && (args.log_metrics || args.log_errors || args.log_dir.is_some()) {
            if let Ok(stats) = logger.get_log_stats() {
                println!(
                    "\u{2713} Logged to structured logs ({} files, {})",
                    stats.total_files,
                    stats.size_human_readable()
                );
            }
        }
    }

    Ok(())
}

/// Save individual response to a file
fn save_individual_response(
    dir: &Path,
    model: &str,
    response: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    fs::create_dir_all(dir)?;
    let filename = format!("{}.txt", model.to_lowercase().replace(' ', "_"));
    let path = dir.join(filename);
    fs::write(&path, response)?;
    Ok(())
}

/// Print a token-usage / latency table for --show-usage
fn print_usage_table(rows: &[(String, Option<u32>, Option<u64>)]) {
    println!("\n{:<20} {:>8}  {:>10}", "Model", "Tokens", "Latency");
    println!("{}", "─".repeat(42));
    for (name, tokens, latency_ms) in rows {
        let tok = tokens.map_or_else(|| "—".to_string(), |t| t.to_string());
        let lat = latency_ms.map_or_else(|| "—".to_string(), |ms| format!("{}ms", ms));
        println!("{:<20} {:>8}  {:>10}", name, tok, lat);
    }
    println!();
}

/// Print available models
fn print_available_models() {
    println!("\u{1f916} Available AI Models\n");
    println!("OpenAI:");
    println!("  \u{2022} gpt-5.4         (flagship reasoning \u{2014} most capable)");
    println!("  \u{2022} o3              (strong reasoning, complex tasks)");
    println!("  \u{2022} gpt-4o          (fast, highly capable \u{2014} default)");
    println!("  \u{2022} gpt-4o-mini     (faster, cheaper)\n");

    println!("Google Gemini:");
    println!("  \u{2022} gemini-3.1-pro        (most capable, reasoning-first)");
    println!("  \u{2022} gemini-2.5-flash      (fast, cost-efficient \u{2014} default)");
    println!("  \u{2022} gemini-2.5-flash-lite (fastest, highest throughput)\n");

    println!("Anthropic Claude:");
    println!("  \u{2022} claude-opus-4-6              (most capable)");
    println!("  \u{2022} claude-sonnet-4-6            (balanced \u{2014} default)");
    println!("  \u{2022} claude-haiku-4-5-20251001    (fast, lightweight)\n");

    println!("\u{1f4a1} Set your preferred models with:");
    println!("   --gpt-model, --gemini-model, --claude-model");
}

/// Check API key configuration and provide setup guidance
fn run_doctor(_args: &Args) -> Result<(), Box<dyn std::error::Error>> {
    println!("\u{1f3e5} ChatDelta Doctor - API Key Configuration Check\n");
    println!("\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\n");

    let mut all_configured = true;
    let mut configured_count = 0;

    let openai_key = env::var("OPENAI_API_KEY")
        .or_else(|_| env::var("CHATGPT_API_KEY"));

    match openai_key {
        Ok(key) => {
            if !key.is_empty() {
                let var_name = if env::var("OPENAI_API_KEY").is_ok() {
                    "OPENAI_API_KEY"
                } else {
                    "CHATGPT_API_KEY"
                };
                println!("\u{2713} OpenAI API Key: Configured ({})", var_name);
                configured_count += 1;
            } else {
                println!("\u{2717} OpenAI API Key: Empty");
                all_configured = false;
            }
        }
        Err(_) => {
            println!("\u{2717} OpenAI API Key: Not found");
            println!("  \u{2192} Get your API key at: https://platform.openai.com/api-keys");
            println!("  \u{2192} Set it with: export OPENAI_API_KEY=your-key-here");
            println!("  \u{2192} Or: export CHATGPT_API_KEY=your-key-here\n");
            all_configured = false;
        }
    }

    match env::var("GEMINI_API_KEY") {
        Ok(key) => {
            if !key.is_empty() {
                println!("\u{2713} Gemini API Key: Configured");
                configured_count += 1;
            } else {
                println!("\u{2717} Gemini API Key: Empty");
                all_configured = false;
            }
        }
        Err(_) => {
            println!("\u{2717} Gemini API Key: Not found");
            println!("  \u{2192} Get your API key at: https://makersuite.google.com/app/apikey");
            println!("  \u{2192} Set it with: export GEMINI_API_KEY=your-key-here\n");
            all_configured = false;
        }
    }

    let anthropic_key = env::var("ANTHROPIC_API_KEY")
        .or_else(|_| env::var("CLAUDE_API_KEY"));

    match anthropic_key {
        Ok(key) => {
            if !key.is_empty() {
                let var_name = if env::var("ANTHROPIC_API_KEY").is_ok() {
                    "ANTHROPIC_API_KEY"
                } else {
                    "CLAUDE_API_KEY"
                };
                println!("\u{2713} Anthropic API Key: Configured ({})", var_name);
                configured_count += 1;
            } else {
                println!("\u{2717} Anthropic API Key: Empty");
                all_configured = false;
            }
        }
        Err(_) => {
            println!("\u{2717} Anthropic API Key: Not found");
            println!("  \u{2192} Get your API key at: https://console.anthropic.com/settings/keys");
            println!("  \u{2192} Set it with: export ANTHROPIC_API_KEY=your-key-here");
            println!("  \u{2192} Or: export CLAUDE_API_KEY=your-key-here\n");
            all_configured = false;
        }
    }

    println!("\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}");
    println!("\u{1f4ca} Summary: {}/3 API keys configured", configured_count);

    if all_configured {
        println!("\n\u{2713} All API keys are configured! You're ready to use ChatDelta.");
        println!("  Run 'chatdelta --test' to verify API connections.");
    } else if configured_count > 0 {
        println!("\n\u{26a0}\u{fe0f}  Some API keys are missing. ChatDelta will work with configured providers.");
        println!("  You need at least one API key to use ChatDelta.");
    } else {
        println!("\n\u{2717} No API keys configured. Please set up at least one API key to use ChatDelta.");
        return Err("No API keys configured".into());
    }

    Ok(())
}

/// Test API connections
async fn test_connections(args: &Args) -> Result<(), Box<dyn std::error::Error>> {
    let mut config_builder = ClientConfig::builder()
        .timeout(Duration::from_secs(args.timeout))
        .retries(0)
        .max_tokens(args.max_tokens);

    if let Some(temp) = args.temperature {
        config_builder = config_builder.temperature(temp);
    }

    if let Some(ref system_prompt) = args.system_prompt {
        config_builder = config_builder.system_message(system_prompt);
    }

    let config = config_builder.build();

    let test_prompt = "Hello, please respond with just 'OK' to confirm you're working.";
    let mut all_passed = true;

    if args.should_use_ai("gpt") {
        let openai_key = env::var("OPENAI_API_KEY")
            .or_else(|_| env::var("CHATGPT_API_KEY"));

        match openai_key {
            Ok(key) => match create_client("openai", &key, &args.gpt_model, config.clone()) {
                Ok(client) => match client.send_prompt(test_prompt).await {
                    Ok(_) => println!("\u{2713} ChatGPT connection successful"),
                    Err(e) => {
                        println!("\u{2717} ChatGPT connection failed: {}", e);
                        all_passed = false;
                    }
                },
                Err(e) => {
                    println!("\u{2717} ChatGPT client creation failed: {}", e);
                    all_passed = false;
                }
            },
            Err(_) => {
                println!("\u{2717} ChatGPT: OPENAI_API_KEY or CHATGPT_API_KEY not set");
                all_passed = false;
            }
        }
    }

    if args.should_use_ai("gemini") {
        match env::var("GEMINI_API_KEY") {
            Ok(key) => match create_client("gemini", &key, &args.gemini_model, config.clone()) {
                Ok(client) => match client.send_prompt(test_prompt).await {
                    Ok(_) => println!("\u{2713} Gemini connection successful"),
                    Err(e) => {
                        println!("\u{2717} Gemini connection failed: {}", e);
                        all_passed = false;
                    }
                },
                Err(e) => {
                    println!("\u{2717} Gemini client creation failed: {}", e);
                    all_passed = false;
                }
            },
            Err(_) => {
                println!("\u{2717} Gemini: GEMINI_API_KEY not set");
                all_passed = false;
            }
        }
    }

    if args.should_use_ai("claude") {
        let anthropic_key = env::var("ANTHROPIC_API_KEY")
            .or_else(|_| env::var("CLAUDE_API_KEY"));

        match anthropic_key {
            Ok(key) => match create_client("claude", &key, &args.claude_model, config.clone()) {
                Ok(client) => match client.send_prompt(test_prompt).await {
                    Ok(_) => println!("\u{2713} Claude connection successful"),
                    Err(e) => {
                        println!("\u{2717} Claude connection failed: {}", e);
                        all_passed = false;
                    }
                },
                Err(e) => {
                    println!("\u{2717} Claude client creation failed: {}", e);
                    all_passed = false;
                }
            },
            Err(_) => {
                println!("\u{2717} Claude: ANTHROPIC_API_KEY or CLAUDE_API_KEY not set");
                all_passed = false;
            }
        }
    }

    if all_passed {
        println!("\n\u{2713} All API connections working properly");
        Ok(())
    } else {
        Err("Some API connections failed".into())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    // Route to debate subcommand if present
    if let Some(command) = args.command {
        let result = match command {
            Commands::Debate(debate_args) | Commands::Deliberate(debate_args) => {
                run_debate(debate_args).await
            }
        };
        if let Err(e) = result {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
        return Ok(());
    }

    if let Err(e) = run(args).await {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
    Ok(())
}

/// Run the debate subcommand
async fn run_debate(args: DebateArgs) -> Result<(), Box<dyn std::error::Error>> {
    use debate::{DebateConfig, DebateProtocol, ModelSpec, Orchestrator};
    use std::str::FromStr;

    args.validate()?;

    // Resolve proposition from --prompt, --prompt-file, or stdin
    let proposition = if let Some(ref p) = args.prompt {
        if p == "-" {
            let mut buf = String::new();
            io::stdin().read_to_string(&mut buf)?;
            let s = buf.trim().to_string();
            if s.is_empty() {
                return Err("No proposition provided via stdin".into());
            }
            s
        } else {
            p.clone()
        }
    } else if let Some(ref path) = args.prompt_file {
        let content = fs::read_to_string(path)
            .map_err(|e| format!("Failed to read prompt file: {}", e))?;
        let s = content.trim().to_string();
        if s.is_empty() {
            return Err("Prompt file is empty".into());
        }
        s
    } else {
        use std::io::IsTerminal;
        if !io::stdin().is_terminal() {
            let mut buf = String::new();
            io::stdin().read_to_string(&mut buf)?;
            let s = buf.trim().to_string();
            if s.is_empty() {
                return Err("No proposition provided via stdin".into());
            }
            s
        } else {
            return Err(
                "No proposition provided. Use --prompt <text>, --prompt-file <path>, or pipe via stdin.".into(),
            );
        }
    };

    // Parse model specs
    let model_a = ModelSpec::from_str(&args.model_a)?;
    let model_b = ModelSpec::from_str(&args.model_b)?;
    let moderator_spec: Option<ModelSpec> = args
        .moderator
        .as_deref()
        .map(ModelSpec::from_str)
        .transpose()?;

    // Parse protocol
    let protocol = DebateProtocol::from_str(&args.protocol)?;

    // Build per-turn client config (focused, shorter responses)
    let mut turn_config_builder = ClientConfig::builder()
        .timeout(Duration::from_secs(args.timeout))
        .retries(args.retries)
        .max_tokens(1024);
    if let Some(temp) = args.temperature {
        turn_config_builder = turn_config_builder.temperature(temp);
    }
    let turn_config = turn_config_builder.build();

    // Build moderator config (more tokens for comprehensive synthesis)
    let mod_config = ClientConfig::builder()
        .timeout(Duration::from_secs(args.timeout))
        .retries(args.retries)
        .max_tokens(2048)
        .build();

    // Create clients
    let client_a = debate::resolve_client(&model_a, turn_config.clone())?;
    let client_b = debate::resolve_client(&model_b, turn_config.clone())?;

    let moderator_client = if let Some(ref spec) = moderator_spec {
        Some(debate::resolve_client(spec, mod_config.clone())?)
    } else {
        let auto = debate::resolve_auto_moderator(mod_config)?;
        if auto.is_none() && !args.quiet {
            eprintln!(
                "Warning: No moderator API key available. \
                 Debate will proceed without moderator synthesis."
            );
        }
        auto
    };

    let debate_config = DebateConfig {
        proposition,
        model_a,
        model_b,
        moderator: moderator_spec,
        rounds: args.rounds,
        protocol,
        max_turn_chars: args.max_turn_chars,
        export_path: args.export.clone(),
    };

    let mut orchestrator = Orchestrator::new(
        debate_config,
        client_a,
        client_b,
        moderator_client,
        args.quiet,
    );

    let transcript = orchestrator.run().await?;

    if let Some(ref export_path) = args.export {
        debate::DebateRenderer::export_markdown(&transcript, export_path)?;
        if !args.quiet {
            println!("Transcript exported to: {}", export_path.display());
        }
    }

    Ok(())
}

/// Run interactive conversation mode
async fn run_conversation_mode(args: &Args) -> Result<(), Box<dyn std::error::Error>> {
    use std::io::Write;

    println!("\u{1f5e8}\u{fe0f}  ChatDelta Conversation Mode");
    println!("Type 'exit' or 'quit' to end the conversation");
    println!("Type 'clear' to reset the conversation history");
    println!("Type 'save' to save the conversation to a file");
    println!();

    // Create client configuration with system_message and retry strategy
    let mut config_builder = ClientConfig::builder()
        .timeout(Duration::from_secs(args.timeout))
        .retries(args.retries)
        .max_tokens(args.max_tokens);

    if let Some(temp) = args.temperature {
        config_builder = config_builder.temperature(temp);
    }

    let retry_strategy = match args.retry_strategy.as_str() {
        "linear" => RetryStrategy::Linear(Duration::from_secs(1)),
        "fixed" => RetryStrategy::Fixed(Duration::from_secs(2)),
        _ => RetryStrategy::Exponential(Duration::from_secs(1)),
    };
    config_builder = config_builder.retry_strategy(retry_strategy);

    // Wire up system prompt into ClientConfig (available since chatdelta 0.4.0)
    if let Some(ref system_prompt) = args.system_prompt {
        config_builder = config_builder.system_message(system_prompt);
        if !args.quiet {
            println!("\u{1f4cb} System prompt active");
        }
    }

    let config = config_builder.build();

    // Create a client (prefer GPT for conversation mode)
    let client: Box<dyn AiClient> = if args.should_use_ai("gpt") {
        let openai_key = env::var("OPENAI_API_KEY")
            .or_else(|_| env::var("CHATGPT_API_KEY"));

        if let Ok(key) = openai_key {
            create_client("openai", &key, &args.gpt_model, config)?
        } else {
            return Err(
                "Conversation mode requires at least one API key (OPENAI_API_KEY or CHATGPT_API_KEY recommended)"
                    .into(),
            );
        }
    } else if args.should_use_ai("gemini") {
        if let Ok(key) = env::var("GEMINI_API_KEY") {
            create_client("gemini", &key, &args.gemini_model, config)?
        } else {
            return Err("Conversation mode requires at least one API key".into());
        }
    } else if args.should_use_ai("claude") {
        let anthropic_key = env::var("ANTHROPIC_API_KEY")
            .or_else(|_| env::var("CLAUDE_API_KEY"));

        if let Ok(key) = anthropic_key {
            create_client("anthropic", &key, &args.claude_model, config)?
        } else {
            return Err("Conversation mode requires at least one API key".into());
        }
    } else {
        return Err("No AI clients available for conversation mode".into());
    };

    // Create a ChatSession
    let mut session = ChatSession::new(client);

    // Load conversation history if specified
    if let Some(ref path) = args.load_conversation {
        let json = fs::read_to_string(path)
            .map_err(|e| format!("Failed to read conversation file '{}': {}", path.display(), e))?;
        let messages: Vec<Message> = serde_json::from_str(&json)
            .map_err(|e| format!("Failed to parse conversation file '{}': {}", path.display(), e))?;
        let count = messages.len();
        session.load_history(messages);
        if !args.quiet {
            println!("\u{1f4c1} Loaded {} messages from: {}", count, path.display());
        }
    }

    // Main conversation loop
    loop {
        print!("> ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim();

        match input.to_lowercase().as_str() {
            "exit" | "quit" => {
                println!("\u{1f44b} Goodbye!");
                break;
            }
            "clear" => {
                session.clear();
                println!("\u{1f504} Conversation cleared");
                continue;
            }
            "save" => {
                if let Some(ref path) = args.save_conversation {
                    let json = serde_json::to_string_pretty(&session.history().messages)?;
                    fs::write(path, &json)?;
                    println!("\u{1f4be} Conversation saved to: {}", path.display());
                } else {
                    println!("\u{26a0}\u{fe0f}  No save path specified. Use --save-conversation <path>");
                }
                continue;
            }
            "" => continue,
            _ => {}
        }

        println!("\u{1f914} Thinking...");

        match session.send(input).await {
            Ok(response) => {
                println!("\n{}\n", response);
            }
            Err(e) => {
                eprintln!("\u{274c} Error: {}", e);
            }
        }
    }

    // Save conversation on exit if requested
    if let Some(ref path) = args.save_conversation {
        let json = serde_json::to_string_pretty(&session.history().messages)?;
        fs::write(path, &json)?;
        if !args.quiet {
            println!("\u{1f4be} Conversation saved to: {}", path.display());
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::{Args, DebateArgs};
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
        let args = Args::try_parse_from(["chatdelta", ""]).expect("Should parse test arguments");
        assert!(args.validate().is_err());
    }

    // This test uses unsafe remove_var but doesn't clear alternative key names
    // (CHATGPT_API_KEY, CLAUDE_API_KEY), so it makes real API calls when keys are
    // present in the environment. Marked ignore; run manually with -- --ignored.
    #[ignore = "unreliable when real API keys are present; use MockClient tests instead"]
    #[tokio::test]
    async fn test_run_missing_env_vars() {
        unsafe {
            env::remove_var("OPENAI_API_KEY");
            env::remove_var("GEMINI_API_KEY");
            env::remove_var("ANTHROPIC_API_KEY");
        }
        let args =
            Args::try_parse_from(["chatdelta", "Test"]).expect("Should parse test arguments");
        let result = run(args).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_debate_args_parsing() {
        let args = Args::try_parse_from([
            "chatdelta",
            "debate",
            "--model-a",
            "openai:gpt-4o",
            "--model-b",
            "anthropic:claude-sonnet-4-6",
            "--prompt",
            "AI will transform education",
        ])
        .expect("Should parse debate arguments");

        assert!(args.command.is_some());
        args.validate().expect("Should pass with subcommand");
    }

    #[test]
    fn test_debate_args_validation() {
        let valid = DebateArgs {
            model_a: "openai:gpt-4o".to_string(),
            model_b: "anthropic:claude-sonnet-4-6".to_string(),
            moderator: None,
            rounds: 1,
            protocol: "moderated-debate".to_string(),
            prompt: Some("Test proposition".to_string()),
            prompt_file: None,
            export: None,
            max_turn_chars: 2000,
            timeout: 120,
            retries: 1,
            temperature: None,
            quiet: false,
        };
        assert!(valid.validate().is_ok());

        let too_many_rounds = DebateArgs {
            rounds: 11,
            ..valid.clone()
        };
        assert!(too_many_rounds.validate().is_err());
    }

    #[test]
    fn test_stream_flag_parsing() {
        let args = Args::try_parse_from(["chatdelta", "--stream", "--only", "claude", "Hello"])
            .expect("Should parse stream flag");
        assert!(args.stream);
        assert_eq!(args.only, vec!["claude"]);
    }

    #[test]
    fn test_show_usage_flag_parsing() {
        let args = Args::try_parse_from(["chatdelta", "--show-usage", "How fast is Rust?"])
            .expect("Should parse --show-usage");
        assert!(args.show_usage);
        args.validate().expect("Should pass validation");
    }

    /// Verifies that print_usage_table handles None metadata gracefully.
    #[test]
    fn test_print_usage_table_none_metadata() {
        // Rows with no metadata (as returned by MockClient's default send_prompt_with_metadata)
        let rows: Vec<(String, Option<u32>, Option<u64>)> = vec![
            ("ChatGPT".to_string(), Some(512), Some(1200)),
            ("Claude".to_string(), None, None),
        ];
        // Just verify it doesn't panic
        print_usage_table(&rows);
    }

    /// Uses MockClient to exercise the execute_parallel_with_metadata code path
    /// end-to-end: metadata is extracted and the usage_rows vec is populated.
    #[tokio::test]
    async fn test_usage_rows_built_from_metadata() {
        use chatdelta::{execute_parallel_with_metadata, MockClient};

        let clients: Vec<Box<dyn AiClient>> = vec![
            Box::new(MockClient::new("gpt", vec![Ok("answer A".to_string())])),
            Box::new(MockClient::new("claude", vec![Ok("answer B".to_string())])),
        ];

        let raw = execute_parallel_with_metadata(clients, "test prompt").await;
        assert_eq!(raw.len(), 2);

        let mut plain = Vec::new();
        let mut usage_rows: Vec<(String, Option<u32>, Option<u64>)> = Vec::new();
        for (name, result) in raw {
            match result {
                Ok(r) => {
                    usage_rows.push((name.clone(), r.metadata.total_tokens, r.metadata.latency_ms));
                    plain.push((name, r.content));
                }
                Err(e) => panic!("unexpected error: {e}"),
            }
        }

        assert_eq!(plain.len(), 2);
        assert_eq!(usage_rows.len(), 2);
        // MockClient returns empty metadata, so tokens/latency are None — verify no panic
        for (_, tokens, latency) in &usage_rows {
            let _ = tokens.map_or_else(|| "—".to_string(), |t| t.to_string());
            let _ = latency.map_or_else(|| "—".to_string(), |ms| format!("{}ms", ms));
        }
    }

    /// Tests conversation session lifecycle using MockClient — no live API keys required.
    #[tokio::test]
    async fn test_conversation_session_load_save_clear() {
        use chatdelta::{ChatSession, Message, MockClient};

        let mock = MockClient::new(
            "mock",
            vec![
                Ok("The capital of France is Paris.".to_string()),
                Ok("Rust was created by Graydon Hoare.".to_string()),
            ],
        );

        let mut session = ChatSession::new(Box::new(mock));

        // Simulate --load-conversation: deserialize JSON into Vec<Message> and call load_history
        let prior: Vec<Message> = vec![
            Message::user("Hi"),
            Message::assistant("Hello! How can I help you?"),
        ];
        let serialized = serde_json::to_string(&prior).unwrap();
        let loaded: Vec<Message> = serde_json::from_str(&serialized).unwrap();
        session.load_history(loaded);

        assert_eq!(session.history().messages.len(), 2);
        assert_eq!(session.history().messages[0].role, "user");

        // Send a message — history grows and mock response is consumed
        let r1 = session.send("What is the capital of France?").await.unwrap();
        assert_eq!(r1, "The capital of France is Paris.");
        assert_eq!(session.history().messages.len(), 4);

        // Simulate --save-conversation: serialize history to JSON and round-trip it
        let saved_json = serde_json::to_string_pretty(&session.history().messages).unwrap();
        let restored: Vec<Message> = serde_json::from_str(&saved_json).unwrap();
        assert_eq!(restored.len(), 4);
        assert_eq!(restored[3].role, "assistant");
        assert_eq!(restored[3].content, "The capital of France is Paris.");

        // 'clear' command resets history
        session.clear();
        assert!(session.history().messages.is_empty());

        // Session is still usable after clear
        let r2 = session.send("Who created Rust?").await.unwrap();
        assert_eq!(r2, "Rust was created by Graydon Hoare.");
        assert_eq!(session.history().messages.len(), 2);
    }
}
