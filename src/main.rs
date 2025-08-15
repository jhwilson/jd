use anyhow::Result;
use clap::{Parser, Subcommand, Args, ValueEnum};
use std::path::PathBuf;

mod model;
mod fs_walk;
mod tsv;
mod preview;
mod state;
mod resolve;
mod mutate;
mod io;
mod ignore;

#[derive(Parser, Debug)]
#[command(name = "jd-helper", version, about = "Filesystem-first JD helper")] 
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Scan(ScanCmd),
    Tree(TreeCmd),
    Preview(PreviewCmd),
    Resolve(ResolveCmd),
    Parent(ParentCmd),
    Codes(CodesCmd),
    New(NewCmd),
    NewInteractive(NewInteractiveCmd),
    Rename(RenameCmd),
    Move(MoveCmd),
    Delete(DeleteCmd),
    Suggest(SuggestCmd),
    Toggle(ToggleCmd),
    WriteIndex(WriteIndexCmd),
    ResetState(ResetStateCmd),
    ExpandAll(ExpandAllCmd),
}

#[derive(Args, Debug)]
struct ScanCmd {
    #[arg(required=true)]
    roots: Vec<PathBuf>,
}

#[derive(Args, Debug)]
struct TreeCmd {
    #[arg(required=true)]
    roots: Vec<PathBuf>,
    #[arg(long)]
    filter: Option<String>,
    #[arg(long)]
    state: Option<PathBuf>,
    #[arg(long, help="List all nodes regardless of fold state")] 
    all: bool,
    #[arg(long, help="Do not auto-expand root level")] 
    collapse_root: bool,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug, ValueEnum)]
enum PreviewType { Dir, File, Link }

#[derive(Args, Debug)]
struct PreviewCmd {
    #[arg(long, value_enum)]
    r#type: PreviewType,
    #[arg(long)]
    path: PathBuf,
}

#[derive(Args, Debug)]
struct ResolveCmd { code: String, #[arg(required=true)] roots: Vec<PathBuf> }

#[derive(Args, Debug)]
struct ParentCmd { id: String, #[arg(required=true)] roots: Vec<PathBuf>, #[arg(long)] path: bool, #[arg(long)] both: bool }

#[derive(Args, Debug)]
struct CodesCmd { #[arg(required=true)] roots: Vec<PathBuf> }

#[derive(Copy, Clone, Eq, PartialEq, Debug, ValueEnum)]
enum NewKind { Dir, File, Link }

#[derive(Args, Debug)]
struct NewCmd {
    #[arg(value_enum)]
    kind: NewKind,
    #[arg(long)]
    parent: String,
    #[arg(long)]
    name: String,
    #[arg(long)]
    url: Option<String>,
    #[arg(long)]
    location: Option<String>,
    #[arg(required=true)]
    roots: Vec<PathBuf>,
}

#[derive(Args, Debug)]
struct NewInteractiveCmd {
    #[arg(long, value_enum)]
    kind: Option<NewKind>,
    #[arg(long, value_name="ID")]
    parent_id: String,
    #[arg(long, value_name="DISPLAY")]
    display: String,
    #[arg(required=true)]
    roots: Vec<PathBuf>,
}

#[derive(Args, Debug)]
struct RenameCmd { #[arg(long)] id: String, #[arg(long)] name: String, #[arg(required=true)] roots: Vec<PathBuf> }

#[derive(Args, Debug)]
struct MoveCmd { #[arg(long)] id: String, #[arg(long)] parent: String, #[arg(required=true)] roots: Vec<PathBuf> }

#[derive(Args, Debug)]
struct DeleteCmd { #[arg(long)] id: String, #[arg(required=true)] roots: Vec<PathBuf> }

#[derive(Args, Debug)]
struct SuggestCmd { #[arg(long)] parent: String, #[arg(required=true)] roots: Vec<PathBuf> }

#[derive(Args, Debug)]
struct ToggleCmd { #[arg(long)] state: PathBuf, #[arg(long)] id: String }

#[derive(Args, Debug)]
struct WriteIndexCmd { #[arg(required=true)] roots: Vec<PathBuf>, #[arg(long)] out: Option<PathBuf> }

#[derive(Args, Debug)]
struct ResetStateCmd { #[arg(long)] state: PathBuf }

#[derive(Args, Debug)]
struct ExpandAllCmd { #[arg(long)] state: PathBuf, #[arg(required=true)] roots: Vec<PathBuf> }

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Scan(cmd) => {
            let tree = fs_walk::scan_roots(&cmd.roots)?;
            println!("{}", serde_json::to_string_pretty(&tree)?);
        }
        Commands::Tree(cmd) => {
            let tree = fs_walk::scan_roots(&cmd.roots)?;
            let state_path = cmd.state;
            let expanded = state::load_state_or_default(state_path.as_ref())?;
            let lines = tsv::flatten_to_tsv(&tree, cmd.filter.as_deref(), &expanded, cmd.all, cmd.collapse_root);
            for l in lines { println!("{}", l); }
        }
        Commands::Preview(cmd) => {
            let out = match cmd.r#type { PreviewType::Dir => preview::preview_dir(&cmd.path), PreviewType::File => preview::preview_file(&cmd.path), PreviewType::Link => preview::preview_link(&cmd.path) }?;
            print!("{}", out);
        }
        Commands::Resolve(cmd) => {
            let tree = fs_walk::scan_roots(&cmd.roots)?;
            let p = resolve::resolve_code_to_path(&tree, &cmd.code)?;
            println!("{}", p.display());
        }
        Commands::Parent(cmd) => {
            let tree = fs_walk::scan_roots(&cmd.roots)?;
            let id = cmd.id;
            if let Some(pid) = model::find_parent_id(&tree, &id) {
                if cmd.path || cmd.both {
                    // find path of parent
                    fn find<'a>(n: &'a model::Node, id: &str) -> Option<&'a model::Node> { if n.id==id { return Some(n);} for c in &n.children { if let Some(x)=find(c,id){return Some(x);} } None }
                    let mut ppath = None;
                    for r in &tree.roots { if let Some(n)=find(r,&pid){ ppath = Some(n.path.clone()); break; } }
                    if cmd.both { println!("{}\t{}", pid, ppath.unwrap_or_default()); }
                    else { println!("{}", ppath.unwrap_or_default()); }
                } else {
                    println!("{}", pid);
                }
            } else { std::process::exit(1); }
        }
        Commands::Codes(cmd) => {
            let tree = fs_walk::scan_roots(&cmd.roots)?;
            for c in model::all_codes(&tree) { println!("{}", c); }
        }
        Commands::New(cmd) => {
            let kind = match cmd.kind { NewKind::Dir => mutate::NewKind::Dir, NewKind::File => mutate::NewKind::File, NewKind::Link => mutate::NewKind::Link };
            mutate::create(&cmd.roots, kind, &cmd.parent, &cmd.name, cmd.url.as_deref(), cmd.location.as_deref())?;
        }
        Commands::NewInteractive(cmd) => {
            let pre = cmd.kind.map(|k| match k { NewKind::Dir => mutate::NewKind::Dir, NewKind::File => mutate::NewKind::File, NewKind::Link => mutate::NewKind::Link });
            mutate::new_interactive_any(&cmd.roots, &cmd.parent_id, &cmd.display, pre)?;
        }
        Commands::Rename(cmd) => { mutate::rename(&cmd.roots, &cmd.id, &cmd.name)?; }
        Commands::Move(cmd) => { mutate::move_node(&cmd.roots, &cmd.id, &cmd.parent)?; }
        Commands::Delete(cmd) => { mutate::delete_node(&cmd.roots, &cmd.id)?; }
        Commands::Suggest(cmd) => {
            let tree = fs_walk::scan_roots(&cmd.roots)?;
            let next = model::suggest_next_code(&tree, &cmd.parent)?;
            println!("{}", next);
        }
        Commands::Toggle(cmd) => {
            let mut st = state::load_state_or_default(Some(&cmd.state))?;
            st.toggle(&cmd.id);
            state::save_state(&cmd.state, &st)?;
        }
        Commands::WriteIndex(cmd) => {
            let tree = fs_walk::scan_roots(&cmd.roots)?;
            let out = io::IndexIo::default().write_index(cmd.out.as_ref(), &tree)?;
            println!("{}", out.display());
        }
        Commands::ResetState(cmd) => {
            let empty = tsv::ExpandedState { expanded: Default::default() };
            state::save_state(&cmd.state, &empty)?;
        }
        Commands::ExpandAll(cmd) => {
            let tree = fs_walk::scan_roots(&cmd.roots)?;
            fn collect_ids(node: &model::Node, out: &mut Vec<String>) {
                let is_dir_like = matches!(node.node_type, model::NodeType::Range | model::NodeType::Category | model::NodeType::ItemDir | model::NodeType::Other);
                if is_dir_like { out.push(node.id.clone()); }
                for ch in &node.children { collect_ids(ch, out); }
            }
            let mut ids = Vec::new();
            for r in &tree.roots { collect_ids(r, &mut ids); }
            let expanded: std::collections::BTreeSet<String> = ids.into_iter().collect();
            let st = tsv::ExpandedState { expanded };
            state::save_state(&cmd.state, &st)?;
        }
    }
    Ok(())
}

// interactive helpers moved into mutate module


