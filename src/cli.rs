use clap::{Args, Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(
    name = "ricardo",
    version,
    about = "Live Ricardo and Tutti listings and photos through your browser",
    arg_required_else_help = true
)]
#[command(
    after_help = "Start: ricardo doctor --json\nAgent guide: ricardo robot-docs guide\nSearch: ricardo search Bambu P2S --max-price 350 --json"
)]
pub struct Cli {
    /// Emit one versioned JSON envelope; diagnostics never pollute stdout.
    #[arg(long, global = true)]
    pub json: bool,
    /// Disable ANSI styling (output is already plain by default).
    #[arg(long, global = true)]
    pub no_color: bool,
    /// Maximum browser request duration in seconds.
    #[arg(long, global = true, default_value_t = 30, value_parser = clap::value_parser!(u64).range(1..=300))]
    pub timeout: u64,
    /// Override the private local state directory.
    #[arg(long, global = true)]
    pub data_dir: Option<PathBuf>,
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Search the current Ricardo interface, with fresh observation timestamps.
    Search(SearchArgs),
    /// Inspect a Ricardo ID/URL or a full Tutti listing URL.
    Inspect { article: String },
    /// Download a listing gallery to numbered local files (Ricardo ID/URL or Tutti URL).
    Images(ImageArgs),
    /// Search and inspect Tutti through the same paced browser bridge.
    Tutti {
        #[command(subcommand)]
        command: TuttiCommand,
    },
    /// Read the signed-in account overview.
    Account,
    /// Read or update favorites; mutations preview unless --yes is supplied.
    #[command(visible_alias = "watchlist")]
    Favorites {
        #[command(subcommand)]
        command: FavoritesCommand,
    },
    /// Read buying offers (not completed orders).
    Buying {
        #[arg(long, default_value_t = 0)]
        offset: u32,
        #[arg(long, default_value_t = 30, value_parser = clap::value_parser!(u32).range(1..=30))]
        limit: u32,
    },
    /// Read your active selling offers.
    Selling,
    /// Read account notifications.
    Notifications,
    /// Fetch a read-only Ricardo interface snapshot for a URL.
    Page { url: String },
    /// Diagnose the local bridge, browser connection, and authentication.
    Doctor,
    /// Discover commands, output schema, limitations, and exit codes.
    Capabilities,
    /// Print the agent handbook without a browser connection.
    RobotDocs {
        #[command(subcommand)]
        command: RobotDocsCommand,
    },
    /// Read saved observations; history never claims current availability.
    History {
        query: Option<String>,
        #[arg(long, default_value_t = 20, value_parser = clap::value_parser!(u32).range(1..=1000))]
        limit: u32,
    },
    /// Manage the connection between the CLI and browser extension.
    Bridge {
        #[command(subcommand)]
        command: BridgeCommand,
    },
    /// Set up the browser extension's native messaging host.
    Extension {
        #[command(subcommand)]
        command: ExtensionCommand,
    },
}

#[derive(Debug, Args)]
pub struct ImageArgs {
    /// Ricardo article ID/URL or a full Tutti listing URL.
    pub article: String,
    /// Parent directory; each run creates its own private subdirectory.
    #[arg(long)]
    pub output_dir: Option<PathBuf>,
    /// Maximum photos to download, in gallery order.
    #[arg(long, default_value_t = 20, value_parser = clap::value_parser!(u32).range(1..=50))]
    pub limit: u32,
    /// Download only this photo (one-based gallery index).
    #[arg(long, value_parser = clap::value_parser!(u32).range(1..=1000), conflicts_with = "limit")]
    pub index: Option<u32>,
}

#[derive(Debug, Subcommand)]
pub enum TuttiCommand {
    /// Search the first result page; selected listings are individually verified.
    Search {
        #[arg(required = true, num_args = 1..)]
        query: Vec<String>,
        #[arg(long, default_value_t = 5, value_parser = clap::value_parser!(u32).range(1..=20))]
        limit: u32,
    },
    /// Inspect a full Tutti listing URL, including all gallery URLs.
    Inspect { url: String },
    /// Download Tutti photos to local files for visual inspection.
    Images(ImageArgs),
}

#[derive(Debug, Args)]
pub struct SearchArgs {
    /// Search words (quoted or unquoted).
    #[arg(required = true, num_args = 1..)]
    pub query: Vec<String>,
    /// Maximum next bid or purchase price in CHF; auction outcome can be higher.
    #[arg(long, value_parser = nonnegative_price)]
    pub max_price: Option<f64>,
    #[arg(long, default_value_t = 5, value_parser = clap::value_parser!(u32).range(1..=100))]
    pub limit: u32,
    #[arg(long, default_value_t = 1, value_parser = clap::value_parser!(u32).range(1..=1000))]
    pub page: u32,
    #[arg(long, value_enum, default_value_t = Sort::Relevance)]
    pub sort: Sort,
    /// Require active listings (the default).
    #[arg(long, default_value_t = true, action = clap::ArgAction::SetTrue, conflicts_with = "include_ended")]
    pub active: bool,
    /// Also allow listings observed as ended; overrides default active filtering.
    #[arg(long)]
    pub include_ended: bool,
    /// Ricardo category identifier.
    #[arg(long)]
    pub category: Option<String>,
    /// Exact product attribute, e.g. printer_3d; sellers may miscategorize accessories.
    #[arg(long)]
    pub product_type: Option<String>,
}

fn nonnegative_price(raw: &str) -> Result<f64, String> {
    let value: f64 = raw
        .parse()
        .map_err(|_| "price must be a number in CHF".to_owned())?;
    if !value.is_finite() || value < 0.0 {
        return Err("price must be finite and nonnegative".to_owned());
    }
    Ok(value)
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum Sort {
    Relevance,
    Price,
    Ending,
}

impl Sort {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Relevance => "relevance",
            Self::Price => "price",
            Self::Ending => "ending",
        }
    }
}

#[derive(Debug, Subcommand)]
pub enum FavoritesCommand {
    /// Read your current favorites.
    List {
        #[arg(long, default_value_t = 0)]
        offset: u32,
        #[arg(long, default_value_t = 50, value_parser = clap::value_parser!(u32).range(1..=50))]
        limit: u32,
    },
    /// Preview adding an article; --yes performs the change.
    Add {
        article: String,
        #[arg(long)]
        yes: bool,
    },
    /// Preview removing an article; --yes performs the change.
    Remove {
        article: String,
        #[arg(long)]
        yes: bool,
    },
}

#[derive(Debug, Subcommand)]
pub enum RobotDocsCommand {
    Guide,
}

#[derive(Debug, Subcommand)]
pub enum BridgeCommand {
    /// Serve a browser worker over JSONL stdin/stdout in the foreground.
    Serve,
    /// Native messaging transport launched by the browser extension.
    #[command(hide = true)]
    Native,
    /// Report bridge connectivity without fetching Ricardo pages.
    Status,
}

#[derive(Debug, Subcommand)]
pub enum ExtensionCommand {
    /// Register the local native host; load the extension separately in Chrome.
    Install {
        /// ID shown on Chrome's extensions page after loading the unpacked extension.
        #[arg(long)]
        extension_id: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn agent_search_handles_unquoted_words_and_global_json() {
        let cli = Cli::try_parse_from([
            "ricardo",
            "search",
            "Bambu",
            "P2S",
            "--max-price",
            "350",
            "--json",
        ])
        .unwrap();
        assert!(cli.json);
        let Command::Search(search) = cli.command else {
            panic!("wrong command")
        };
        assert_eq!(search.query.join(" "), "Bambu P2S");
        assert_eq!(search.max_price, Some(350.0));
        assert!(search.active);
    }
    #[test]
    fn include_ended_is_not_blocked_by_active_default() {
        assert!(Cli::try_parse_from(["ricardo", "search", "P2S", "--include-ended"]).is_ok());
        assert!(
            Cli::try_parse_from(["ricardo", "search", "P2S", "--include-ended", "--active"])
                .is_err()
        );
    }
    #[test]
    fn reject_nonfinite_prices_and_unbounded_requests() {
        for price in ["NaN", "inf", "-1"] {
            assert!(
                Cli::try_parse_from(["ricardo", "search", "P2S", "--max-price", price]).is_err()
            );
        }
        assert!(Cli::try_parse_from(["ricardo", "--timeout", "0", "doctor"]).is_err());
        assert!(Cli::try_parse_from(["ricardo", "search", "P2S", "--limit", "0"]).is_err());
    }
    #[test]
    fn mutations_are_previews_by_default() {
        let cli = Cli::try_parse_from(["ricardo", "favorites", "add", "1234567890"]).unwrap();
        assert!(matches!(
            cli.command,
            Command::Favorites {
                command: FavoritesCommand::Add { yes: false, .. }
            }
        ));
    }

    #[test]
    fn structured_product_filter_and_plain_output_are_accepted() {
        let cli = Cli::try_parse_from([
            "ricardo",
            "search",
            "Bambu",
            "P2S",
            "--product-type",
            "printer_3d",
            "--no-color",
        ])
        .unwrap();
        assert!(cli.no_color);
        let Command::Search(search) = cli.command else {
            panic!("wrong command")
        };
        assert_eq!(search.product_type.as_deref(), Some("printer_3d"));
    }

    #[test]
    fn native_transport_has_an_explicit_command() {
        let cli = Cli::try_parse_from(["ricardo", "bridge", "native"]).unwrap();
        assert!(matches!(
            cli.command,
            Command::Bridge {
                command: BridgeCommand::Native
            }
        ));
    }

    #[test]
    fn account_pagination_is_bounded() {
        let cli = Cli::try_parse_from([
            "ricardo",
            "favorites",
            "list",
            "--offset",
            "50",
            "--limit",
            "20",
        ])
        .unwrap();
        assert!(matches!(
            cli.command,
            Command::Favorites {
                command: FavoritesCommand::List {
                    offset: 50,
                    limit: 20
                }
            }
        ));
        let cli = Cli::try_parse_from(["ricardo", "buying", "--offset", "30"]).unwrap();
        assert!(matches!(
            cli.command,
            Command::Buying {
                offset: 30,
                limit: 30
            }
        ));
        assert!(Cli::try_parse_from(["ricardo", "buying", "--limit", "31"]).is_err());
        assert!(Cli::try_parse_from(["ricardo", "favorites", "list", "--limit", "51"]).is_err());
    }
}
