//! Molecular structure types and PDB I/O.
//!
//! Provides Atom, Bond, Residue, and Molecule structs for representing
//! molecular systems, with factory methods for standard biomolecules
//! and simplified PDB-format parsing/output.

use crate::core::types::Scalar;
use crate::domains::molbio::forcefield::{HarmonicBond, LennardJones, Vec3};
use crate::domains::molbio::physics::element_mass;

// ──────────────────────────────────────────────
// Atom
// ──────────────────────────────────────────────

/// A single atom in a molecular system.
#[derive(Debug, Clone)]
pub struct Atom {
    /// Atom serial number.
    pub serial: u32,
    /// Atom name (e.g., "CA", "CB", "N").
    pub name: String,
    /// Residue name (e.g., "ALA", "GLY").
    pub resname: String,
    /// Chain identifier.
    pub chain: char,
    /// Residue sequence number.
    pub resseq: u32,
    /// Element symbol (e.g., "C", "N", "O").
    pub element: String,
    /// 3D position (Angstrom).
    pub position: Vec3,
    /// Velocity (Angstrom/ps).
    pub velocity: Vec3,
    /// Atomic mass (amu).
    pub mass: Scalar,
    /// Partial charge (e).
    pub charge: Scalar,
    /// Lennard-Jones parameters (optional).
    pub lj: Option<LennardJones>,
}

impl Atom {
    /// Create a new atom with minimal fields.
    pub fn new(name: &str, element: &str, x: Scalar, y: Scalar, z: Scalar) -> Self {
        let mass = element_mass(element).unwrap_or(12.011);
        Self {
            serial: 0,
            name: name.to_string(),
            resname: "UNK".to_string(),
            chain: 'A',
            resseq: 1,
            element: element.to_string(),
            position: Vec3::new(x, y, z),
            velocity: Vec3::zero(),
            mass,
            charge: 0.0,
            lj: None,
        }
    }
}

// ──────────────────────────────────────────────
// Bond
// ──────────────────────────────────────────────

/// A covalent bond between two atoms.
#[derive(Debug, Clone)]
pub struct Bond {
    /// Index of first atom.
    pub i: usize,
    /// Index of second atom.
    pub j: usize,
    /// Bond order (1=single, 2=double, 3=triple).
    pub order: u8,
    /// Harmonic bond parameters (optional).
    pub params: Option<HarmonicBond>,
}

impl Bond {
    pub fn new(i: usize, j: usize, order: u8) -> Self {
        Self {
            i,
            j,
            order,
            params: None,
        }
    }
}

// ──────────────────────────────────────────────
// Residue
// ──────────────────────────────────────────────

/// A residue (amino acid or nucleotide) in a molecular chain.
#[derive(Debug, Clone)]
pub struct Residue {
    /// Residue name (three-letter code).
    pub name: String,
    /// Chain identifier.
    pub chain: char,
    /// Sequence number.
    pub seqnum: u32,
    /// Indices of atoms belonging to this residue.
    pub atoms: Vec<usize>,
}

impl Residue {
    pub fn new(name: &str, chain: char, seqnum: u32) -> Self {
        Self {
            name: name.to_string(),
            chain,
            seqnum,
            atoms: Vec::new(),
        }
    }
}

// ──────────────────────────────────────────────
// Molecule
// ──────────────────────────────────────────────

/// A complete molecular system with atoms, bonds, residues, and force field.
#[derive(Debug, Clone)]
pub struct Molecule {
    /// Molecule name.
    pub name: String,
    /// Atoms in the molecule.
    pub atoms: Vec<Atom>,
    /// Bonds between atoms.
    pub bonds: Vec<Bond>,
    /// Residue definitions.
    pub residues: Vec<Residue>,
}

impl Molecule {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            atoms: Vec::new(),
            bonds: Vec::new(),
            residues: Vec::new(),
        }
    }

    /// Add an atom and return its index.
    pub fn add_atom(&mut self, atom: Atom) -> usize {
        let idx = self.atoms.len();
        self.atoms.push(atom);
        idx
    }

    /// Add a bond between two atoms.
    pub fn add_bond(&mut self, i: usize, j: usize, order: u8) {
        self.bonds.push(Bond::new(i, j, order));
    }

    /// Number of atoms.
    pub fn num_atoms(&self) -> usize {
        self.atoms.len()
    }

    /// Extract atomic positions as a slice of Vec3.
    pub fn atom_positions(&self) -> Vec<Vec3> {
        self.atoms.iter().map(|a| a.position).collect()
    }

    /// Compute the center of mass.
    pub fn center_of_mass(&self) -> Vec3 {
        let mut com = Vec3::zero();
        let mut total_mass = 0.0;
        for atom in &self.atoms {
            com = com.add(&atom.position.scale(atom.mass));
            total_mass += atom.mass;
        }
        if total_mass > 0.0 {
            com.scale(1.0 / total_mass)
        } else {
            Vec3::zero()
        }
    }

    /// Compute the radius of gyration.
    pub fn radius_of_gyration(&self) -> Scalar {
        let com = self.center_of_mass();
        let mut sum_sq = 0.0;
        let mut total_mass = 0.0;
        for atom in &self.atoms {
            let d = atom.position.distance(&com);
            sum_sq += atom.mass * d * d;
            total_mass += atom.mass;
        }
        if total_mass > 0.0 {
            (sum_sq / total_mass).sqrt()
        } else {
            0.0
        }
    }

    /// Compute the pairwise distance matrix (Å).
    pub fn distance_matrix(&self) -> Vec<Vec<Scalar>> {
        let n = self.atoms.len();
        let mut dm = vec![vec![0.0; n]; n];
        #[allow(clippy::needless_range_loop)]
        for i in 0..n {
            let row_i = &self.atoms[i];
            #[allow(clippy::needless_range_loop)]
            for j in (i + 1)..n {
                let d = row_i.position.distance(&self.atoms[j].position);
                dm[i][j] = d;
                dm[j][i] = d;
            }
        }
        dm
    }

    /// Parse a simplified PDB-format line and add the atom.
    pub fn add_from_pdb_line(&mut self, line: &str) -> Result<usize, String> {
        if line.len() < 54 {
            return Err("PDB line too short".to_string());
        }
        let serial: u32 = line[6..11]
            .trim()
            .parse()
            .map_err(|e| format!("bad serial: {e}"))?;
        let name = line[12..16].trim().to_string();
        let resname = line[17..20].trim().to_string();
        let chain = line[21..22].chars().next().unwrap_or('A');
        let resseq: u32 = line[22..26]
            .trim()
            .parse()
            .map_err(|e| format!("bad resseq: {e}"))?;
        let x: Scalar = line[30..38]
            .trim()
            .parse()
            .map_err(|e| format!("bad x: {e}"))?;
        let y: Scalar = line[38..46]
            .trim()
            .parse()
            .map_err(|e| format!("bad y: {e}"))?;
        let z: Scalar = line[46..54]
            .trim()
            .parse()
            .map_err(|e| format!("bad z: {e}"))?;
        let element = if line.len() > 76 {
            line[76..78].trim().to_string()
        } else {
            name.chars().next().unwrap_or('C').to_string()
        };

        let mass = element_mass(&element).unwrap_or(12.011);
        let mut atom = Atom {
            serial,
            name,
            resname,
            chain,
            resseq,
            element,
            position: Vec3::new(x, y, z),
            velocity: Vec3::zero(),
            mass,
            charge: 0.0,
            lj: None,
        };

        // Try to set default LJ from element
        if let Some((sigma, epsilon)) =
            crate::domains::molbio::physics::lj_parameters(&atom.element)
        {
            atom.lj = Some(LennardJones::new(sigma, epsilon));
        }

        let idx = self.atoms.len();
        self.atoms.push(atom);
        Ok(idx)
    }

    /// Generate a simplified PDB-format string.
    pub fn to_pdb_string(&self) -> String {
        let mut lines = Vec::new();
        lines.push("REMARK  Generated by SCIcoRS molbio module".to_string());
        for atom in &self.atoms {
            let serial = format!("{:>5}", atom.serial);
            let name = format!(" {:<3}", atom.name);
            let resname = format!("{:<3}", atom.resname);
            let chain = atom.chain;
            let resseq = format!("{:>4}", atom.resseq);
            let x = format!("{:>8.3}", atom.position.x);
            let y = format!("{:>8.3}", atom.position.y);
            let z = format!("{:>8.3}", atom.position.z);
            let element = format!("{:>2}", atom.element);
            lines.push(format!(
                "ATOM  {serial} {name} {resname} {chain}{resseq}    {x}{y}{z}  1.00  0.00          {element}"
            ));
        }
        lines.push("END".to_string());
        lines.join("\n")
    }

    // ──────────────────────────────────────────
    // Factory Methods
    // ──────────────────────────────────────────

    /// Build a standard alanine amino acid.
    pub fn alanine() -> Self {
        let mut mol = Molecule::new("Alanine");
        // N, CA, CB, C, O, HA, HB1, HB2, HB3, H1, H2, H3 (simplified)
        let atoms = vec![
            ("N", "N", 0.000, 0.000, 0.000),
            ("CA", "C", 1.460, 0.000, 0.000),
            ("CB", "C", 2.020, 1.430, 0.000),
            ("C", "C", 1.960, -0.750, 1.230),
            ("O", "O", 2.020, -1.980, 1.220),
            ("HA", "H", 1.800, -0.500, -0.900),
            ("HB1", "H", 3.110, 1.430, 0.000),
            ("HB2", "H", 1.680, 1.960, 0.890),
            ("HB3", "H", 1.680, 1.960, -0.890),
            ("H1", "H", -0.400, 0.500, 0.100),
            ("H2", "H", -0.300, -0.500, 0.100),
        ];
        for (name, elem, x, y, z) in atoms {
            let mut atom = Atom::new(name, elem, x, y, z);
            let idx = mol.atoms.len() as u32 + 1;
            atom.serial = idx;
            atom.resname = "ALA".to_string();
            atom.resseq = 1;
            if let Some((sigma, epsilon)) = crate::domains::molbio::physics::lj_parameters(elem) {
                atom.lj = Some(LennardJones::new(sigma, epsilon));
            }
            mol.atoms.push(atom);
        }
        // Bonds
        mol.add_bond(0, 1, 1); // N-CA
        mol.add_bond(1, 2, 1); // CA-CB
        mol.add_bond(1, 3, 1); // CA-C
        mol.add_bond(3, 4, 2); // C=O
        mol.add_bond(1, 5, 1); // CA-HA
        mol.add_bond(2, 6, 1); // CB-HB1
        mol.add_bond(2, 7, 1); // CB-HB2
        mol.add_bond(2, 8, 1); // CB-HB3
        mol.add_bond(0, 9, 1); // N-H1
        mol.add_bond(0, 10, 1); // N-H2

        // Residue
        let mut res = Residue::new("ALA", 'A', 1);
        res.atoms = (0..mol.atoms.len()).collect();
        mol.residues.push(res);
        mol
    }

    /// Build a simple poly-AT DNA (4 base pairs) — simplified model.
    pub fn dna_at4() -> Self {
        let mut mol = Molecule::new("Poly-AT(4bp)");
        // Simplified DNA backbone + bases (just C, N, O, P atoms)
        // This is a highly simplified representation.
        let elements = [
            ("P", "P", 0.0, 0.0, 0.0),
            ("O5'", "O", 3.0, 0.0, 0.0),
            ("C5'", "C", 3.5, 1.2, 0.0),
            ("C4'", "C", 3.0, 2.0, 1.0),
            ("C3'", "C", 1.8, 1.8, 1.5),
            ("O3'", "O", 1.6, 2.2, 2.8),
            ("C1'", "C", 1.2, 2.8, 0.8),
            ("N1", "N", -0.1, 2.4, 1.2),
            ("C2", "C", -0.6, 1.4, 0.7),
            ("N3", "N", -1.8, 1.0, 1.1),
            ("C4", "C", -2.4, 1.7, 2.1),
            ("C5", "C", -1.8, 2.7, 2.6),
            ("C6", "C", -0.6, 3.1, 2.2),
            ("N6", "N", 0.0, 4.0, 2.7),
            ("N9", "N", 2.0, 3.2, 1.2),
            ("C8", "C", 3.2, 3.4, 1.6),
            ("N7", "N", 3.4, 2.8, 2.6),
        ];

        for (name, elem, x, y, z) in &elements {
            let mut atom = Atom::new(name, elem, *x, *y, *z);
            let idx = mol.atoms.len() as u32 + 1;
            atom.serial = idx;
            atom.resname = "DA".to_string();
            atom.resseq = 1;
            if let Some((sigma, epsilon)) = crate::domains::molbio::physics::lj_parameters(elem) {
                atom.lj = Some(LennardJones::new(sigma, epsilon));
            }
            mol.atoms.push(atom);
        }

        let mut res = Residue::new("DA", 'A', 1);
        res.atoms = (0..mol.atoms.len()).collect();
        mol.residues.push(res);
        mol
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_molecule_creation() {
        let mut mol = Molecule::new("test");
        let idx = mol.add_atom(Atom::new("CA", "C", 0.0, 0.0, 0.0));
        assert_eq!(idx, 0);
        assert_eq!(mol.num_atoms(), 1);
    }

    #[test]
    fn test_center_of_mass() {
        let mut mol = Molecule::new("test");
        mol.add_atom(Atom::new("CA", "C", 0.0, 0.0, 0.0));
        mol.add_atom(Atom::new("CB", "C", 2.0, 0.0, 0.0));
        let com = mol.center_of_mass();
        assert!((com.x - 1.0).abs() < 0.01);
        assert!(com.y.abs() < 0.01);
        assert!(com.z.abs() < 0.01);
    }

    #[test]
    fn test_radius_of_gyration() {
        let mut mol = Molecule::new("test");
        mol.add_atom(Atom::new("CA", "C", 0.0, 0.0, 0.0));
        mol.add_atom(Atom::new("CB", "C", 2.0, 0.0, 0.0));
        let rg = mol.radius_of_gyration();
        assert!(rg > 0.0);
    }

    #[test]
    fn test_distance_matrix() {
        let mut mol = Molecule::new("test");
        mol.add_atom(Atom::new("CA", "C", 0.0, 0.0, 0.0));
        mol.add_atom(Atom::new("CB", "C", 3.0, 4.0, 0.0));
        let dm = mol.distance_matrix();
        assert!((dm[0][1] - 5.0).abs() < 1e-10);
        assert!((dm[1][0] - 5.0).abs() < 1e-10);
    }

    #[test]
    fn test_alanine_creation() {
        let ala = Molecule::alanine();
        assert_eq!(ala.num_atoms(), 11);
        assert_eq!(ala.residues.len(), 1);
        assert_eq!(ala.bonds.len(), 10);
    }

    #[test]
    fn test_dna_creation() {
        let dna = Molecule::dna_at4();
        assert_eq!(dna.num_atoms(), 17);
    }

    #[test]
    fn test_pdb_roundtrip() {
        let ala = Molecule::alanine();
        let pdb = ala.to_pdb_string();
        assert!(pdb.starts_with("REMARK"));
        assert!(pdb.contains("ATOM"));
        assert!(pdb.contains("END"));
    }

    #[test]
    fn test_add_bond() {
        let mut mol = Molecule::new("test");
        mol.add_atom(Atom::new("CA", "C", 0.0, 0.0, 0.0));
        mol.add_atom(Atom::new("CB", "C", 1.5, 0.0, 0.0));
        mol.add_bond(0, 1, 1);
        assert_eq!(mol.bonds.len(), 1);
        assert_eq!(mol.bonds[0].order, 1);
    }
}
