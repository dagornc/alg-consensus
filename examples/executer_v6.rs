//! CLI autonome du nouveau protocole ; ne change pas la CLI historique.
use consensus_rs::{
    protocole_v6::{Config, Moteur},
    sim::N,
};
use std::{
    error::Error,
    fs,
    io::{Read, Write},
};
fn lire(path: &str) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut data = Vec::new();
    fs::File::open(path)?
        .take(64 * 1024 * 1024 + 1)
        .read_to_end(&mut data)?;
    if data.len() > 64 * 1024 * 1024 {
        return Err("fichier trop volumineux".into());
    }
    Ok(data)
}
fn run() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args().skip(1);
    let mut config = None;
    let mut resume = None;
    let mut checkpoint = None;
    let mut seed = 42;
    let mut steps = u64::MAX;
    let mut aggregate = true;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--help" => {
                println!(
                    "--config FILE | --resume FILE ; --seed U64 ; --steps U64 ; --checkpoint NEW_FILE ; --raw ; --default-config\nSans configuration : 30 agents, états initiaux (identifiant,0). La sauvegarde refuse tout écrasement."
                );
                return Ok(());
            }
            "--default-config" => {
                println!("{}", serde_json::to_string_pretty(&Config::default())?);
                return Ok(());
            }
            "--raw" => aggregate = false,
            "--config" => config = Some(args.next().ok_or("config manquante")?),
            "--resume" => resume = Some(args.next().ok_or("reprise manquante")?),
            "--checkpoint" => checkpoint = Some(args.next().ok_or("destination manquante")?),
            "--seed" => seed = args.next().ok_or("graine manquante")?.parse()?,
            "--steps" => steps = args.next().ok_or("nombre de pas manquant")?.parse()?,
            _ => return Err(format!("argument inconnu : {a}").into()),
        }
    }
    if resume.is_some() && (config.is_some() || !aggregate || seed != 42) {
        return Err("reprise incompatible avec config/raw/seed modifiés".into());
    }
    let mut m = if let Some(path) = resume {
        Moteur::reprendre(&lire(&path)?)?
    } else {
        let c = if let Some(path) = config {
            serde_json::from_slice(&lire(&path)?)?
        } else {
            Config::default()
        };
        Moteur::nouveau(
            c,
            seed,
            std::array::from_fn::<_, N, _>(|i| Some((i as i32, 0))),
            aggregate,
        )?
    };
    for _ in 0..steps {
        if m.observation().termine {
            break;
        }
        m.pas();
    }
    if let Some(path) = checkpoint {
        let bytes = m.checkpoint()?;
        let mut f = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)?;
        f.write_all(&bytes)?;
        f.sync_all()?;
    }
    println!("{}", serde_json::to_string_pretty(&m.observation())?);
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
