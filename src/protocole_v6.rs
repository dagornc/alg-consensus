//! Spécification v7 : canal unique, reprise exacte, calendrier agrégé.
//! Nouveau contrat : pas de parité revendiquée avec les raccourcis réseau v2–v5.
use crate::sim::{Etat, GRID_H, GRID_W, N, voisins};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

const FORMAT: u32 = 1;
const MAX_CHECKPOINT: usize = 64 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Evenement {
    pub tour: u64,
    pub agent: usize,
    /// None = retrait ; Some = nouvelle incarnation avec état explicitement injecté.
    pub etat: Option<Etat>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub horizon: u64,
    pub perte: f64,
    pub latence: u64,
    pub duplication: u32,
    pub fin_partition: u64,
    pub quiescence: bool,
    pub seuil_stable: u64,
    pub reveil: u64,
    /// Un pair supplémentaire, déterministe et tournant, tous les K tours.
    /// Zéro désactive la reconnexion ; aucune lecture distante directe.
    pub sonde: u64,
    pub budget_messages: u64,
    pub capacite_messages: u64,
    pub evenements: Vec<Evenement>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            horizon: 200,
            perte: 0.0,
            latence: 2,
            duplication: 1,
            fin_partition: 0,
            quiescence: true,
            seuil_stable: 8,
            reveil: 16,
            sonde: 4,
            budget_messages: 1_000_000,
            capacite_messages: 100_000,
            evenements: Vec::new(),
        }
    }
}

impl Config {
    pub fn valider(&self) -> Result<(), String> {
        if !self.perte.is_finite()
            || !(0.0..=1.0).contains(&self.perte)
            || self.horizon > 100_000
            || self.latence > 10_000
            || self.duplication > 64
            || self.reveil == 0
            || self.budget_messages > 10_000_000
            || self.capacite_messages > 100_000
            || self.evenements.len() > 10_000
            || self.fin_partition > self.horizon
        {
            return Err("configuration hors limites v6".into());
        }
        let mut previous = None;
        for e in &self.evenements {
            let key = (e.tour, e.agent);
            if e.tour == 0
                || e.tour > self.horizon
                || e.agent >= N
                || previous.is_some_and(|p| p >= key)
            {
                return Err(
                    "événements : ordre strict (tour,agent), un événement/agent/tour".into(),
                );
            }
            previous = Some(key);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Compteurs {
    pub tentatives: u64,
    pub perdues: u64,
    pub partition: u64,
    pub inactif: u64,
    pub saturation: u64,
    pub livrees: u64,
    pub incarnation: u64,
    pub en_vol: u64,
}

impl Compteurs {
    pub fn conserves(&self) -> bool {
        let sum = [
            self.perdues,
            self.partition,
            self.inactif,
            self.saturation,
            self.livrees,
            self.incarnation,
            self.en_vol,
        ]
        .into_iter()
        .try_fold(0u64, u64::checked_add);
        sum == Some(self.tentatives)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Paquet {
    valeur: Etat,
    nombre: u64,
    epoch: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Lot {
    arrivee: u64,
    cases: [Option<Paquet>; N],
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Observation {
    pub tour: u64,
    pub etats: [Option<Etat>; N],
    pub compteurs: Compteurs,
    /// Accord sur le couple complet, constat centralisé, pas détection distribuée.
    pub accord: bool,
    pub termine: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Moteur {
    format: u32,
    config: Config,
    tour: u64,
    etats: [Option<Etat>; N],
    epochs: [u64; N],
    stable: [u64; N],
    rng: u64,
    compteurs: Compteurs,
    file: VecDeque<Lot>,
    /// Mode brut : oracle de stockage, même protocole, une copie/lot.
    agrege: bool,
}

impl Moteur {
    pub fn nouveau(
        config: Config,
        seed: u64,
        etats: [Option<Etat>; N],
        agrege: bool,
    ) -> Result<Self, String> {
        config.valider()?;
        Ok(Self {
            format: FORMAT,
            config,
            tour: 0,
            etats,
            epochs: [0; N],
            stable: [0; N],
            rng: seed,
            compteurs: Compteurs::default(),
            file: VecDeque::new(),
            agrege,
        })
    }

    // SplitMix64 : PRNG de simulation versionné, non cryptographique.
    fn random(&mut self) -> f64 {
        self.rng = self.rng.wrapping_add(0x9e3779b97f4a7c15);
        let mut z = self.rng;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
        z ^= z >> 31;
        (z >> 11) as f64 / 9_007_199_254_740_992.0
    }

    pub fn observation(&self) -> Observation {
        let mut values = self.etats.iter().flatten();
        let accord = values
            .next()
            .is_some_and(|first| values.all(|v| v == first));
        Observation {
            tour: self.tour,
            etats: self.etats,
            compteurs: self.compteurs,
            accord,
            termine: self.tour >= self.config.horizon
                || self.compteurs.tentatives >= self.config.budget_messages,
        }
    }

    fn livrer(&mut self) {
        while self
            .file
            .front()
            .is_some_and(|lot| lot.arrivee <= self.tour)
        {
            let lot = self.file.pop_front().unwrap();
            for (d, p) in lot.cases.into_iter().enumerate() {
                if let Some(p) = p {
                    self.compteurs.en_vol -= p.nombre;
                    if p.epoch != self.epochs[d] || self.etats[d].is_none() {
                        self.compteurs.incarnation += p.nombre;
                    } else {
                        self.compteurs.livrees += p.nombre;
                        self.etats[d] = self.etats[d].max(Some(p.valeur));
                    }
                }
            }
        }
    }

    fn envoyer(&mut self, source: usize, dest: usize, valeur: Etat) {
        for _ in 0..self.config.duplication {
            if self.compteurs.tentatives == self.config.budget_messages {
                return;
            }
            self.compteurs.tentatives += 1;
            let random = self.random(); // un tirage pour CHAQUE tentative
            if self.tour <= self.config.fin_partition
                && (source % GRID_W < GRID_W / 2) != (dest % GRID_W < GRID_W / 2)
            {
                self.compteurs.partition += 1;
            } else if self.etats[dest].is_none() {
                self.compteurs.inactif += 1;
            } else if random < self.config.perte {
                self.compteurs.perdues += 1;
            } else if self.compteurs.en_vol == self.config.capacite_messages {
                self.compteurs.saturation += 1;
            } else {
                let arrivee = self.tour + self.config.latence;
                let p = Paquet {
                    valeur,
                    nombre: 1,
                    epoch: self.epochs[dest],
                };
                if self.agrege && self.file.back().is_some_and(|lot| lot.arrivee == arrivee) {
                    let case = &mut self.file.back_mut().unwrap().cases[dest];
                    if let Some(previous) = case {
                        debug_assert_eq!(previous.epoch, p.epoch);
                        previous.valeur = previous.valeur.max(valeur);
                        previous.nombre += 1;
                    } else {
                        *case = Some(p);
                    }
                } else {
                    let mut cases = [None; N];
                    cases[dest] = Some(p);
                    self.file.push_back(Lot { arrivee, cases });
                }
                self.compteurs.en_vol += 1;
            }
        }
    }

    /// Une période complète. Les événements futurs empêchent l'arrêt sur simple accord.
    /// Budget épuisé : arrêt des émissions au message exact, puis fin de la période.
    pub fn pas(&mut self) -> Observation {
        if self.observation().termine {
            return self.observation();
        }
        self.tour += 1;
        let start = self
            .config
            .evenements
            .partition_point(|e| e.tour < self.tour);
        for e in self.config.evenements[start..]
            .iter()
            .take_while(|e| e.tour == self.tour)
        {
            self.etats[e.agent] = e.etat;
            self.epochs[e.agent] += 1;
            self.stable[e.agent] = 0;
        }
        let avant = self.etats;
        self.livrer();
        let emission = self.etats; // aucune propagation instantanée en cascade
        for (i, value) in emission.into_iter().enumerate() {
            let Some(value) = value else {
                continue;
            };
            let changed = avant[i] != self.etats[i];
            let awake = !self.config.quiescence
                || changed
                || self.stable[i] < self.config.seuil_stable
                || self.tour % self.config.reveil == 0;
            let probe = self.config.sonde > 0 && self.tour % self.config.sonde == 0;
            let mut cibles = if awake {
                voisins(i, GRID_W, GRID_H)
            } else {
                Vec::with_capacity(1)
            };
            if probe {
                let offset = ((self.tour / self.config.sonde - 1) % (N as u64 - 1) + 1) as usize;
                let dest = (i + offset) % N;
                if !cibles.contains(&dest) {
                    cibles.push(dest);
                }
            }
            for dest in cibles {
                self.envoyer(i, dest, value);
            }
        }
        self.livrer(); // latence zéro : livrer après toutes les émissions
        for (i, before) in avant.into_iter().enumerate() {
            self.stable[i] = if self.etats[i].is_some() && before == self.etats[i] {
                self.stable[i] + 1
            } else {
                0
            };
        }
        debug_assert!(self.compteurs.conserves());
        self.observation()
    }

    pub fn terminer(&mut self) -> Observation {
        while !self.observation().termine {
            self.pas();
        }
        self.observation()
    }

    pub fn checkpoint(&self) -> Result<Vec<u8>, String> {
        let bytes = serde_json::to_vec(self).map_err(|e| e.to_string())?;
        if bytes.len() > MAX_CHECKPOINT {
            return Err("checkpoint trop volumineux".into());
        }
        Ok(bytes)
    }

    pub fn reprendre(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > MAX_CHECKPOINT {
            return Err("checkpoint trop volumineux".into());
        }
        let m: Self = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
        m.config.valider()?;
        let mut epochs = [0u64; N];
        for e in m.config.evenements.iter().filter(|e| e.tour <= m.tour) {
            epochs[e.agent] += 1;
        }
        if m.format != FORMAT
            || m.tour > m.config.horizon
            || !m.compteurs.conserves()
            || m.compteurs.tentatives > m.config.budget_messages
            || m.compteurs.en_vol > m.config.capacite_messages
            || m.stable.iter().any(|s| *s > m.tour)
            || m.epochs != epochs
        {
            return Err("checkpoint incohérent".into());
        }
        let mut last = m.tour;
        let mut total = 0u64;
        for lot in &m.file {
            if lot.arrivee <= m.tour
                || lot.arrivee < last
                || (m.agrege && lot.arrivee == last)
                || lot.arrivee > m.tour + m.config.latence
                || lot.cases.iter().all(Option::is_none)
            {
                return Err("calendrier invalide".into());
            }
            last = lot.arrivee;
            if !m.agrege && lot.cases.iter().flatten().count() != 1 {
                return Err("lot brut invalide".into());
            }
            for (d, p) in lot.cases.iter().enumerate() {
                if let Some(p) = p {
                    if p.nombre == 0 || p.epoch > m.epochs[d] || (!m.agrege && p.nombre != 1) {
                        return Err("paquet invalide".into());
                    }
                    total = total.checked_add(p.nombre).ok_or("compteur débordant")?;
                }
            }
        }
        if total != m.compteurs.en_vol {
            return Err("file et compteur incompatibles".into());
        }
        Ok(m)
    }

    pub fn lots_stockes(&self) -> usize {
        self.file.len()
    }
}
