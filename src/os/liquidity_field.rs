// ============================================================
// NAVIER-STOKES LIQUIDITY FIELD (ℒ₄)
// ============================================================
// Partial differential equation solver for liquidity dynamics
// Vorticity calculation for market turbulence
// Real-time field updates
// ============================================================

use super::*;
use std::f64::consts::PI;

/// Field parameters
#[derive(Debug, Clone)]
pub struct FieldParams {
    pub viscosity: f64,      // ν - liquidity viscosity
    pub diffusion: f64,      // D - diffusion coefficient
    pub dt: f64,             // Time step
    pub dx: f64,             // Space step
    pub num_points: usize,   // Grid resolution
    pub boundary_type: BoundaryType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundaryType {
    Periodic,
    Dirichlet,
    Neumann,
    Reflecting,
}

impl Default for FieldParams {
    fn default() -> Self {
        Self {
            viscosity: 0.01,
            diffusion: 0.001,
            dt: 0.0001,
            dx: 0.01,
            num_points: 256,
            boundary_type: BoundaryType::Reflecting,
        }
    }
}

/// Navier-Stokes liquidity field solver
pub struct NavierStokesLiquidity {
    params: FieldParams,
    u: Vec<f64>,      // Velocity field
    u_prev: Vec<f64>, // Previous velocity
    p: Vec<f64>,      // Pressure field
    f: Vec<f64>,      // External forcing (f_liq)
    vorticity: Vec<f64>,
    laplacian: Vec<f64>,
}

impl NavierStokesLiquidity {
    /// Create new liquidity field solver
    pub fn new(num_points: usize, viscosity: f64, diffusion: f64) -> Self {
        let params = FieldParams {
            num_points,
            viscosity,
            diffusion,
            ..Default::default()
        };
        
        Self {
            u: vec![0.0; num_points],
            u_prev: vec![0.0; num_points],
            p: vec![0.0; num_points],
            f: vec![0.0; num_points],
            vorticity: vec![0.0; num_points],
            laplacian: vec![0.0; num_points],
            params,
        }
    }
    
    /// Update field using Navier-Stokes equation
    /// ∂ₜu + (u·∇)u = -∇p + ν∇²u + f_liq
    pub fn update_field(&mut self, field: &[f64], pressure: &[f64]) -> (Vec<f64>, Vec<f64>) {
        let n = self.params.num_points.min(field.len()).min(pressure.len());
        
        // Copy input
        self.u[..n].copy_from_slice(&field[..n]);
        self.p[..n].copy_from_slice(&pressure[..n]);
        self.u_prev.copy_from_slice(&self.u);
        
        // Compute advection term (u·∇)u
        let advection = self.compute_advection();
        
        // Compute pressure gradient ∇p
        let pressure_grad = self.compute_pressure_gradient();
        
        // Compute diffusion term ν∇²u
        let diffusion = self.compute_diffusion();
        
        // Update velocity field
        for i in 0..n {
            // ∂u/∂t = -advection - ∇p + ν∇²u + f
            let du_dt = -advection[i] - pressure_grad[i] + self.params.viscosity * diffusion[i] + self.f[i];
            self.u[i] += du_dt * self.params.dt;
        }
        
        // Apply boundary conditions
        self.apply_boundary_conditions();
        
        // Compute vorticity ω = ∇ × u
        self.compute_vorticity();
        
        (self.u.clone(), self.vorticity.clone())
    }
    
    /// Compute advection term (u·∇)u
    fn compute_advection(&self) -> Vec<f64> {
        let mut advection = vec![0.0; self.params.num_points];
        let dx = self.params.dx;
        
        for i in 1..self.params.num_points - 1 {
            // Upwind differencing for stability
            if self.u[i] > 0.0 {
                advection[i] = self.u[i] * (self.u[i] - self.u[i-1]) / dx;
            } else {
                advection[i] = self.u[i] * (self.u[i+1] - self.u[i]) / dx;
            }
        }
        
        advection
    }
    
    /// Compute pressure gradient ∇p
    fn compute_pressure_gradient(&self) -> Vec<f64> {
        let mut grad = vec![0.0; self.params.num_points];
        let dx = self.params.dx;
        
        for i in 1..self.params.num_points - 1 {
            grad[i] = (self.p[i+1] - self.p[i-1]) / (2.0 * dx);
        }
        
        grad
    }
    
    /// Compute diffusion term ∇²u (Laplacian)
    fn compute_diffusion(&self) -> Vec<f64> {
        let mut laplacian = vec![0.0; self.params.num_points];
        let dx2 = self.params.dx * self.params.dx;
        
        for i in 1..self.params.num_points - 1 {
            laplacian[i] = (self.u[i+1] - 2.0 * self.u[i] + self.u[i-1]) / dx2;
        }
        
        laplacian
    }
    
    /// Compute vorticity ω = ∇ × u (2D curl in 1D becomes gradient)
    fn compute_vorticity(&mut self) {
        let dx = self.params.dx;
        
        for i in 1..self.params.num_points - 1 {
            self.vorticity[i] = (self.u[i+1] - self.u[i-1]) / (2.0 * dx);
        }
        
        // Boundaries
        self.vorticity[0] = self.vorticity[1];
        self.vorticity[self.params.num_points - 1] = self.vorticity[self.params.num_points - 2];
    }
    
    /// Apply boundary conditions
    fn apply_boundary_conditions(&mut self) {
        let n = self.params.num_points;
        
        match self.params.boundary_type {
            BoundaryType::Periodic => {
                self.u[0] = self.u[n - 2];
                self.u[n - 1] = self.u[1];
            }
            BoundaryType::Dirichlet => {
                self.u[0] = 0.0;
                self.u[n - 1] = 0.0;
            }
            BoundaryType::Neumann => {
                self.u[0] = self.u[1];
                self.u[n - 1] = self.u[n - 2];
            }
            BoundaryType::Reflecting => {
                self.u[0] = -self.u[1];
                self.u[n - 1] = -self.u[n - 2];
            }
        }
    }
    
    /// Apply external forcing f_liq (market orders)
    pub fn apply_forcing(&mut self, forcing: &[f64]) {
        let n = forcing.len().min(self.params.num_points);
        self.f[..n].copy_from_slice(&forcing[..n]);
    }
    
    /// Get current vorticity field
    pub fn vorticity(&self) -> &[f64] {
        &self.vorticity
    }
    
    /// Get velocity field
    pub fn velocity(&self) -> &[f64] {
        &self.u
    }
    
    /// Get field energy
    pub fn total_energy(&self) -> f64 {
        self.u.iter().map(|&u| u * u).sum::<f64>() / 2.0
    }
    
    /// Get enstrophy (vorticity squared)
    pub fn enstrophy(&self) -> f64 {
        self.vorticity.iter().map(|&w| w * w).sum::<f64>() / 2.0
    }
    
    /// Detect liquidity vortex (market turbulence)
    pub fn detect_vortex(&self, threshold: f64) -> Vec<usize> {
        self.vorticity.iter()
            .enumerate()
            .filter(|(_, &w)| w.abs() > threshold)
            .map(|(i, _)| i)
            .collect()
    }
}

/// Liquidity field analyzer
pub struct LiquidityFieldAnalyzer {
    history: VecDeque<Vec<f64>>,
    max_history: usize,
}

impl LiquidityFieldAnalyzer {
    pub fn new(max_history: usize) -> Self {
        Self {
            history: VecDeque::with_capacity(max_history),
            max_history,
        }
    }
    
    pub fn record(&mut self, field: &[f64]) {
        self.history.push_back(field.to_vec());
        while self.history.len() > self.max_history {
            self.history.pop_front();
        }
    }
    
    /// Compute field divergence
    pub fn divergence(&self, field: &[f64]) -> Vec<f64> {
        let mut div = vec![0.0; field.len()];
        for i in 1..field.len() - 1 {
            div[i] = field[i+1] - field[i-1];
        }
        div
    }
    
    /// Detect shock (sudden liquidity change)
    pub fn detect_shock(&self, threshold: f64) -> Option<usize> {
        if self.history.len() < 2 {
            return None;
        }
        
        let current = self.history.back().unwrap();
        let previous = &self.history[self.history.len() - 2];
        
        let diff: f64 = current.iter()
            .zip(previous.iter())
            .map(|(c, p)| (c - p).abs())
            .sum();
        
        if diff > threshold {
            Some(self.history.len() - 1)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_liquidity_field() {
        let mut field = NavierStokesLiquidity::new(100, 0.01, 0.001);
        
        let initial = vec![0.1; 100];
        let pressure = vec![1.0; 100];
        
        let (new_field, vorticity) = field.update_field(&initial, &pressure);
        
        assert_eq!(new_field.len(), 100);
        assert_eq!(vorticity.len(), 100);
    }
}// ============================================================
// NAVIER-STOKES LIQUIDITY FIELD (ℒ₄)
// ============================================================
// Partial differential equation solver for liquidity dynamics
// Vorticity calculation for market turbulence
// Real-time field updates
// ============================================================

use super::*;
use std::f64::consts::PI;

/// Field parameters
#[derive(Debug, Clone)]
pub struct FieldParams {
    pub viscosity: f64,      // ν - liquidity viscosity
    pub diffusion: f64,      // D - diffusion coefficient
    pub dt: f64,             // Time step
    pub dx: f64,             // Space step
    pub num_points: usize,   // Grid resolution
    pub boundary_type: BoundaryType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundaryType {
    Periodic,
    Dirichlet,
    Neumann,
    Reflecting,
}

impl Default for FieldParams {
    fn default() -> Self {
        Self {
            viscosity: 0.01,
            diffusion: 0.001,
            dt: 0.0001,
            dx: 0.01,
            num_points: 256,
            boundary_type: BoundaryType::Reflecting,
        }
    }
}

/// Navier-Stokes liquidity field solver
pub struct NavierStokesLiquidity {
    params: FieldParams,
    u: Vec<f64>,      // Velocity field
    u_prev: Vec<f64>, // Previous velocity
    p: Vec<f64>,      // Pressure field
    f: Vec<f64>,      // External forcing (f_liq)
    vorticity: Vec<f64>,
    laplacian: Vec<f64>,
}

impl NavierStokesLiquidity {
    /// Create new liquidity field solver
    pub fn new(num_points: usize, viscosity: f64, diffusion: f64) -> Self {
        let params = FieldParams {
            num_points,
            viscosity,
            diffusion,
            ..Default::default()
        };
        
        Self {
            u: vec![0.0; num_points],
            u_prev: vec![0.0; num_points],
            p: vec![0.0; num_points],
            f: vec![0.0; num_points],
            vorticity: vec![0.0; num_points],
            laplacian: vec![0.0; num_points],
            params,
        }
    }
    
    /// Update field using Navier-Stokes equation
    /// ∂ₜu + (u·∇)u = -∇p + ν∇²u + f_liq
    pub fn update_field(&mut self, field: &[f64], pressure: &[f64]) -> (Vec<f64>, Vec<f64>) {
        let n = self.params.num_points.min(field.len()).min(pressure.len());
        
        // Copy input
        self.u[..n].copy_from_slice(&field[..n]);
        self.p[..n].copy_from_slice(&pressure[..n]);
        self.u_prev.copy_from_slice(&self.u);
        
        // Compute advection term (u·∇)u
        let advection = self.compute_advection();
        
        // Compute pressure gradient ∇p
        let pressure_grad = self.compute_pressure_gradient();
        
        // Compute diffusion term ν∇²u
        let diffusion = self.compute_diffusion();
        
        // Update velocity field
        for i in 0..n {
            // ∂u/∂t = -advection - ∇p + ν∇²u + f
            let du_dt = -advection[i] - pressure_grad[i] + self.params.viscosity * diffusion[i] + self.f[i];
            self.u[i] += du_dt * self.params.dt;
        }
        
        // Apply boundary conditions
        self.apply_boundary_conditions();
        
        // Compute vorticity ω = ∇ × u
        self.compute_vorticity();
        
        (self.u.clone(), self.vorticity.clone())
    }
    
    /// Compute advection term (u·∇)u
    fn compute_advection(&self) -> Vec<f64> {
        let mut advection = vec![0.0; self.params.num_points];
        let dx = self.params.dx;
        
        for i in 1..self.params.num_points - 1 {
            // Upwind differencing for stability
            if self.u[i] > 0.0 {
                advection[i] = self.u[i] * (self.u[i] - self.u[i-1]) / dx;
            } else {
                advection[i] = self.u[i] * (self.u[i+1] - self.u[i]) / dx;
            }
        }
        
        advection
    }
    
    /// Compute pressure gradient ∇p
    fn compute_pressure_gradient(&self) -> Vec<f64> {
        let mut grad = vec![0.0; self.params.num_points];
        let dx = self.params.dx;
        
        for i in 1..self.params.num_points - 1 {
            grad[i] = (self.p[i+1] - self.p[i-1]) / (2.0 * dx);
        }
        
        grad
    }
    
    /// Compute diffusion term ∇²u (Laplacian)
    fn compute_diffusion(&self) -> Vec<f64> {
        let mut laplacian = vec![0.0; self.params.num_points];
        let dx2 = self.params.dx * self.params.dx;
        
        for i in 1..self.params.num_points - 1 {
            laplacian[i] = (self.u[i+1] - 2.0 * self.u[i] + self.u[i-1]) / dx2;
        }
        
        laplacian
    }
    
    /// Compute vorticity ω = ∇ × u (2D curl in 1D becomes gradient)
    fn compute_vorticity(&mut self) {
        let dx = self.params.dx;
        
        for i in 1..self.params.num_points - 1 {
            self.vorticity[i] = (self.u[i+1] - self.u[i-1]) / (2.0 * dx);
        }
        
        // Boundaries
        self.vorticity[0] = self.vorticity[1];
        self.vorticity[self.params.num_points - 1] = self.vorticity[self.params.num_points - 2];
    }
    
    /// Apply boundary conditions
    fn apply_boundary_conditions(&mut self) {
        let n = self.params.num_points;
        
        match self.params.boundary_type {
            BoundaryType::Periodic => {
                self.u[0] = self.u[n - 2];
                self.u[n - 1] = self.u[1];
            }
            BoundaryType::Dirichlet => {
                self.u[0] = 0.0;
                self.u[n - 1] = 0.0;
            }
            BoundaryType::Neumann => {
                self.u[0] = self.u[1];
                self.u[n - 1] = self.u[n - 2];
            }
            BoundaryType::Reflecting => {
                self.u[0] = -self.u[1];
                self.u[n - 1] = -self.u[n - 2];
            }
        }
    }
    
    /// Apply external forcing f_liq (market orders)
    pub fn apply_forcing(&mut self, forcing: &[f64]) {
        let n = forcing.len().min(self.params.num_points);
        self.f[..n].copy_from_slice(&forcing[..n]);
    }
    
    /// Get current vorticity field
    pub fn vorticity(&self) -> &[f64] {
        &self.vorticity
    }
    
    /// Get velocity field
    pub fn velocity(&self) -> &[f64] {
        &self.u
    }
    
    /// Get field energy
    pub fn total_energy(&self) -> f64 {
        self.u.iter().map(|&u| u * u).sum::<f64>() / 2.0
    }
    
    /// Get enstrophy (vorticity squared)
    pub fn enstrophy(&self) -> f64 {
        self.vorticity.iter().map(|&w| w * w).sum::<f64>() / 2.0
    }
    
    /// Detect liquidity vortex (market turbulence)
    pub fn detect_vortex(&self, threshold: f64) -> Vec<usize> {
        self.vorticity.iter()
            .enumerate()
            .filter(|(_, &w)| w.abs() > threshold)
            .map(|(i, _)| i)
            .collect()
    }
}

/// Liquidity field analyzer
pub struct LiquidityFieldAnalyzer {
    history: VecDeque<Vec<f64>>,
    max_history: usize,
}

impl LiquidityFieldAnalyzer {
    pub fn new(max_history: usize) -> Self {
        Self {
            history: VecDeque::with_capacity(max_history),
            max_history,
        }
    }
    
    pub fn record(&mut self, field: &[f64]) {
        self.history.push_back(field.to_vec());
        while self.history.len() > self.max_history {
            self.history.pop_front();
        }
    }
    
    /// Compute field divergence
    pub fn divergence(&self, field: &[f64]) -> Vec<f64> {
        let mut div = vec![0.0; field.len()];
        for i in
