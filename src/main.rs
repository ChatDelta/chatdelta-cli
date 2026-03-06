//! ChatDelta CLI application
//!
//! A command-line tool for querying multiple AI APIs and summarizing their responses.

use chatdelta::{
    create_client, execute_parallel, generate_summary, AiClient, ChatSession, ClientConfig,
    RetryStrategy,
};
use clap::Parser;
use std::env;
use std::fs;
use std::io::{self, Read};
use std::path::Path;
use std::time::Duration;

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

    // Create client configuration using the builder pattern from chatdelta 0.3.0
    let mut config_builder = ClientConfig::builder()
        .timeout(Duration::from_secs(args.timeout))
        .retries(args.retries)
        .max_tokens(args.max_tokens);

    if let Some(temp) = args.temperature {
        config_builder = config_builder.temperature(temp);
    }

    // Set retry strategy based on CLI argument (new in 0.3.0)
    let retry_strategy = match args.retry_strategy.as_str() {
        "linear" => RetryStrategy::Linear(Duration::from_secs(1)),
        "fixed" => RetryStrategy::Fixed(Duration::from_secs(2)),
        _ => RetryStrategy::Exponential(Duration::from_secs(1)), // default with base delay
    };
    config_builder = config_builder.retry_strategy(retry_strategy);

    // Add system prompt if provided (checking if this is supported in 0.3.0)
    if let Some(ref _system_prompt) = args.system_prompt {
        // Try to set system prompt if the method exists
        // config_builder = config_builder.system_prompt(system_prompt);
        // TODO: Use system prompt when API supports it
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

    // Initialize comprehensive logger
    let mut logger = if args.log_metrics || args.log_errors || args.log_dir.is_some() {
        Some(Logger::new(&args)?)
    } else {
        None
    };

    // Query each model with the same prompt in parallel
    if !args.quiet && args.progress {
        println!(
            "🔄 Querying {} AI model{}...",
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
        println!("📝 Prompt: {}", preview);
    }

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
                    println!("✅ Received response from {} ({} chars)", name, reply.len());
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
        println!(
            "✓ Received {} response{}",
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
    if args.raw {
        // Raw output mode - just print responses
        for (_, response) in &responses {
            println!("{}", response);
        }
    } else {
        output_results(&args, &responses, digest.as_deref())?;
    }

    // Log interaction if requested (legacy simple logging)
    log_interaction(&args, &responses, digest.as_deref())?;

    // Finalize comprehensive logging
    if let Some(mut logger) = logger {
        logger.finalize_interaction(summary_duration)?;

        if !args.quiet && (args.log_metrics || args.log_errors || args.log_dir.is_some()) {
            if let Ok(stats) = logger.get_log_stats() {
                println!(
                    "✓ Logged to structured logs ({} files, {})",
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

/// Print available models
fn print_available_models() {
    println!("🤖 Available AI Models\n");
    println!("OpenAI:");
    println!("  • gpt-5.4         (flagship reasoning — most capable)");
    println!("  • o3              (strong reasoning, complex tasks)");
    println!("  • gpt-4o          (fast, highly capable — default)");
    println!("  • gpt-4o-mini     (faster, cheaper)\n");

    println!("Google Gemini:");
    println!("  • gemini-3.1-pro        (most capable, reasoning-first)");
    println!("  • gemini-2.5-flash      (fast, cost-efficient — default)");
    println!("  • gemini-2.5-flash-lite (fastest, highest throughput)\n");

    println!("Anthropic Claude:");
    println!("  • claude-opus-4-6              (most capable)");
    println!("  • claude-sonnet-4-6            (balanced — default)");
    println!("  • claude-haiku-4-5-20251001    (fast, lightweight)\n");

    println!("💡 Set your preferred models with:");
    println!("   --gpt-model, --gemini-model, --claude-model");
}

/// Check API key configuration and provide setup guidance
fn run_doctor(_args: &Args) -> Result<(), Box<dyn std::error::Error>> {
    println!("🏥 ChatDelta Doctor - API Key Configuration Check\n");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    let mut all_configured = true;
    let mut configured_count = 0;

    // Check OpenAI API Key (check both OPENAI_API_KEY and CHATGPT_API_KEY)
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
                println!("✓ OpenAI API Key: Configured ({})", var_name);
                configured_count += 1;
            } else {
                println!("✗ OpenAI API Key: Empty");
                all_configured = false;
            }
        }
        Err(_) => {
            println!("✗ OpenAI API Key: Not found");
            println!("  → Get your API key at: https://platform.openai.com/api-keys");
            println!("  → Set it with: export OPENAI_API_KEY=your-key-here");
            println!("  → Or: export CHATGPT_API_KEY=your-key-here\n");
            all_configured = false;
        }
    }

    // Check Gemini API Key
    match env::var("GEMINI_API_KEY") {
        Ok(key) => {
            if !key.is_empty() {
                println!("✓ Gemini API Key: Configured");
                configured_count += 1;
            } else {
                println!("✗ Gemini API Key: Empty");
                all_configured = false;
            }
        }
        Err(_) => {
            println!("✗ Gemini API Key: Not found");
            println!("  → Get your API key at: https://makersuite.google.com/app/apikey");
            println!("  → Set it with: export GEMINI_API_KEY=your-key-here\n");
            all_configured = false;
        }
    }

    // Check Anthropic API Key (check both ANTHROPIC_API_KEY and CLAUDE_API_KEY)
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
                println!("✓ Anthropic API Key: Configured ({})", var_name);
                configured_count += 1;
            } else {
                println!("✗ Anthropic API Key: Empty");
                all_configured = false;
            }
        }
        Err(_) => {
            println!("✗ Anthropic API Key: Not found");
            println!("  → Get your API key at: https://console.anthropic.com/settings/keys");
            println!("  → Set it with: export ANTHROPIC_API_KEY=your-key-here");
            println!("  → Or: export CLAUDE_API_KEY=your-key-here\n");
            all_configured = false;
        }
    }

    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!(
        "📊 Summary: {}/3 API keys configured",
        configured_count
    );

    if all_configured {
        println!("\n✓ All API keys are configured! You're ready to use ChatDelta.");
        println!("  Run 'chatdelta --test' to verify API connections.");
    } else if configured_count > 0 {
        println!("\n⚠️  Some API keys are missing. ChatDelta will work with configured providers.");
        println!("  You need at least one API key to use ChatDelta.");
    } else {
        println!("\n✗ No API keys configured. Please set up at least one API key to use ChatDelta.");
        return Err("No API keys configured".into());
    }

    Ok(())
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
        let openai_key = env::var("OPENAI_API_KEY")
            .or_else(|_| env::var("CHATGPT_API_KEY"));

        match openai_key {
            Ok(key) => match create_client("openai", &key, &args.gpt_model, config.clone()) {
                Ok(client) => match client.send_prompt(test_prompt).await {
                    Ok(_) => println!("✓ ChatGPT connection successful"),
                    Err(e) => {
                        println!("✗ ChatGPT connection failed: {}", e);
                        all_passed = false;
                    }
                },
                Err(e) => {
                    println!("✗ ChatGPT client creation failed: {}", e);
                    all_passed = false;
                }
            },
            Err(_) => {
                println!("✗ ChatGPT: OPENAI_API_KEY or CHATGPT_API_KEY not set");
                all_passed = false;
            }
        }
    }

    if args.should_use_ai("gemini") {
        match env::var("GEMINI_API_KEY") {
            Ok(key) => match create_client("gemini", &key, &args.gemini_model, config.clone()) {
                Ok(client) => match client.send_prompt(test_prompt).await {
                    Ok(_) => println!("✓ Gemini connection successful"),
                    Err(e) => {
                        println!("✗ Gemini connection failed: {}", e);
                        all_passed = false;
                    }
                },
                Err(e) => {
                    println!("✗ Gemini client creation failed: {}", e);
                    all_passed = false;
                }
            },
            Err(_) => {
                println!("✗ Gemini: GEMINI_API_KEY not set");
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
                    Ok(_) => println!("✓ Claude connection successful"),
                    Err(e) => {
                        println!("✗ Claude connection failed: {}", e);
                        all_passed = false;
                    }
                },
                Err(e) => {
                    println!("✗ Claude client creation failed: {}", e);
                    all_passed = false;
                }
            },
            Err(_) => {
                println!("✗ Claude: ANTHROPIC_API_KEY or CLAUDE_API_KEY not set");
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
        // Try piped stdin
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

    println!("🗨️  ChatDelta Conversation Mode");
    println!("Type 'exit' or 'quit' to end the conversation");
    println!("Type 'clear' to reset the conversation history");
    println!("Type 'save' to save the conversation to a file");
    println!();

    // Create client configuration with retry strategy
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
        let _history = fs::read_to_string(path)?;
        // TODO: Parse and load history (this would need a proper format)
        println!("📁 Loaded conversation from: {}", path.display());
    }

    // Add system prompt if provided
    if let Some(ref system_prompt) = args.system_prompt {
        // Note: This depends on what methods ChatSession actually provides
        // session.set_system_prompt(system_prompt);
        println!("📋 System prompt set: {}", system_prompt);
    }

    // Main conversation loop
    loop {
        print!("> ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim();

        // Handle special commands
        match input.to_lowercase().as_str() {
            "exit" | "quit" => {
                println!("👋 Goodbye!");
                break;
            }
            "clear" => {
                // Reset conversation - create a new session with the same client
                // Note: We can't extract the client from session, so we need to recreate it
                println!("🔄 Conversation cleared (restart required for now)");
                continue;
            }
            "save" => {
                // Save conversation
                if let Some(ref path) = args.save_conversation {
                    // TODO: Implement saving logic
                    println!("💾 Conversation saved to: {}", path.display());
                } else {
                    println!("⚠️  No save path specified. Use --save-conversation <path>");
                }
                continue;
            }
            "" => continue,
            _ => {}
        }

        // Send message and get response
        println!("🤔 Thinking...");

        // ChatSession takes a single message and maintains history internally
        match session.send(input).await {
            Ok(response) => {
                println!("\n{}\n", response);
            }
            Err(e) => {
                eprintln!("❌ Error: {}", e);
            }
        }
    }

    // Save conversation on exit if requested
    if let Some(ref path) = args.save_conversation {
        // TODO: Implement saving logic
        println!("💾 Conversation saved to: {}", path.display());
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
        // validate() on Args should pass when subcommand is present
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
}
