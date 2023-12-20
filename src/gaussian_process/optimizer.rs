//! Parameter optimization.
//!
//! The fit of the parameters is done by gradient descent (using the ADAM algorithm) on the gradient of the marginal log-likelihood
//! (which let us use all the data without bothering with cross-validation).
//!
//! If the kernel can be rescaled, we use ideas from [Fast methods for training Gaussian processes on large datasets](https://arxiv.org/pdf/1604.01250.pdf)
//! to rescale the kernel at each step with the optimal magnitude which has the effect of fitting the noise without computing its gradient.
//!
//! Otherwise we fit the noise in log-scale as its magnitude matters more than its precise value.

use super::GaussianProcess;
use crate::algebra::{make_cholesky_cov_matrix, make_gradient_covariance_matrices};
use crate::parameters::{kernel::Kernel, prior::Prior};
use chrono::{Duration, Utc};

impl<KernelType: Kernel, PriorType: Prior> GaussianProcess<KernelType, PriorType> {
    //-------------------------------------------------------------------------------------------------
    // NON-SCALABLE KERNEL

    /// Computes the gradient of the marginal likelihood for the current value of each parameter.
    /// The produced vector contains the gradient per kernel parameter followed by the gradient for the noise parameter.
    fn gradient_marginal_likelihood(&self) -> Vec<f64> {
        // formula: 1/2 ( transpose(alpha) * dp * alpha - trace(K^-1 * dp) )
        // K = cov(train,train)
        // alpha = K^-1 * output
        // dp = gradient(K, parameter)

        // Needed for the per parameter gradient computation.
        let cov_inv = self.covmat_cholesky.inverse();
        let alpha = &cov_inv * self.training_outputs.as_vector();

        // Loop over the gradient matrix for each parameter.
        let mut results = vec![];
        for cov_gradient in make_gradient_covariance_matrices(&self.training_inputs.as_matrix(), &self.kernel)
        {
            // transpose(alpha) * cov_gradient * alpha
            let data_fit: f64 = cov_gradient
                .column_iter()
                .zip(alpha.iter())
                .map(|(col, alpha_col)| alpha.dot(&col) * alpha_col)
                .sum();

            // trace(cov_inv * cov_gradient)
            let complexity_penalty: f64 =
                cov_inv.row_iter().zip(cov_gradient.column_iter()).map(|(c, d)| c.tr_dot(&d)).sum();

            results.push((data_fit - complexity_penalty) / 2.);
        }

        // Adds the noise parameter.
        // gradient(K, noise) = 2*noise*Id
        let data_fit = alpha.dot(&alpha);
        let complexity_penalty = cov_inv.trace();
        let noise_gradient = self.noise * (data_fit - complexity_penalty);
        results.push(noise_gradient);

        results
    }

    /// Fit parameters using a gradient descent algorithm.
    ///
    /// Runs for a maximum of `max_iter` iterations (100 is a good default value).
    /// Stops prematurely if all the components of the gradient go below `convergence_fraction` time the value of their respectively parameter (0.05 is a good default value).
    /// Stops prematurely if the runtime exceeds `max_time`.
    ///
    /// The `noise` parameter is fitted in log-scale as its magnitude matters more than its precise value.
    pub(super) fn optimize_parameters(
        &mut self,
        max_iter: usize,
        convergence_fraction: f64,
        max_time: Duration,
    ) {
        // use the ADAM gradient descent algorithm
        // see [optimizing-gradient-descent](https://ruder.io/optimizing-gradient-descent/)
        // for a good point on current gradient descent algorithms

        // Constant parameters.
        let beta1 = 0.9;
        let beta2 = 0.999;
        let epsilon = 1e-8;
        let learning_rate = 0.1;

        let mut parameters: Vec<_> = self
            .kernel
            .get_parameters()
            .iter()
            .map(|&p| if p == 0. { epsilon } else { p }) // Insures no parameter is 0 (which would block the algorithm).
            .collect();
        parameters.push(self.noise.ln()); // Adds noise in log-space.
        let mut mean_grad = vec![0.; parameters.len()];
        let mut var_grad = vec![0.; parameters.len()];

        let time_start = Utc::now();
        for i in 1..=max_iter {
            let mut gradients = self.gradient_marginal_likelihood();
            if let Some(noise_grad) = gradients.last_mut() {
                // Corrects gradient of noise for log-space.
                *noise_grad *= self.noise
            }

            let mut had_significant_progress = false;
            for p in 0..parameters.len() {
                mean_grad[p] = beta1 * mean_grad[p] + (1. - beta1) * gradients[p];
                var_grad[p] = beta2 * var_grad[p] + (1. - beta2) * gradients[p].powi(2);
                let bias_corrected_mean = mean_grad[p] / (1. - beta1.powi(i as i32));
                let bias_corrected_variance = var_grad[p] / (1. - beta2.powi(i as i32));
                let delta = learning_rate * bias_corrected_mean / (bias_corrected_variance.sqrt() + epsilon);
                had_significant_progress |= delta.abs() > convergence_fraction;
                parameters[p] *= 1. + delta;
            }

            // Sets parameters.
            self.kernel.set_parameters(&parameters);
            if let Some(noise) = parameters.last() {
                // Gets out of log-space before setting noise.
                self.noise = noise.exp()
            }

            // Fits model.
            self.covmat_cholesky = make_cholesky_cov_matrix(
                &self.training_inputs.as_matrix(),
                &self.kernel,
                self.noise,
                self.cholesky_epsilon,
            );

            if (!had_significant_progress) || (Utc::now().signed_duration_since(time_start) > max_time) {
                //println!("Iterations:{}", i);
                break;
            };
        }

        /*println!("Fit done. likelihood:{} parameters:{:?} noise:{:e}",
        self.likelihood(),
        parameters,
        self.noise);*/
    }

    //-------------------------------------------------------------------------------------------------
    // SCALABLE KERNEL

    /// Returns a couple containing the optimal scale for the kernel+noise (which is used to optimize the noise)
    /// plus a vector containing the gradient per kernel parameter (but NOT the gradient for the noise parameter).
    ///
    /// See [Fast methods for training Gaussian processes on large datasets](https://arxiv.org/pdf/1604.01250.pdf)
    /// for the formula used to compute the scale and the modification to the gradient.
    fn scaled_gradient_marginal_likelihood(&self) -> (f64, Vec<f64>) {
        // formula:
        // gradient = 1/2 ( transpose(alpha) * dp * alpha / scale - trace(K^-1 * dp) )
        // scale = transpose(output) * K^-1 * output / n
        // K = cov(train,train)
        // alpha = K^-1 * output
        // dp = gradient(K, parameter)

        // Needed for the per parameter gradient computation.
        let cov_inv = self.covmat_cholesky.inverse();
        let training_output = self.training_outputs.as_vector();
        let alpha = &cov_inv * training_output;

        // Scaling for the kernel.
        let scale = training_output.dot(&alpha) / (training_output.nrows() as f64);

        // Loop on the gradient matrix for each parameter.
        let mut results = vec![];
        for cov_gradient in make_gradient_covariance_matrices(&self.training_inputs.as_matrix(), &self.kernel)
        {
            // transpose(alpha) * cov_gradient * alpha / scale
            // NOTE: This quantity is divided by the scale which is not the case for the unscaled gradient.
            let data_fit = cov_gradient
                .column_iter()
                .zip(alpha.iter())
                .map(|(col, alpha_col)| alpha.dot(&col) * alpha_col)
                .sum::<f64>()
                / scale;

            // trace(cov_inv * cov_gradient)
            let complexity_penalty: f64 =
                cov_inv.row_iter().zip(cov_gradient.column_iter()).map(|(c, d)| c.tr_dot(&d)).sum();

            results.push((data_fit - complexity_penalty) / 2.);
        }

        // adds the noise parameter
        // gradient(K, noise) = 2*noise*Id
        /*let data_fit = alpha.dot(&alpha) / scale;
        let complexity_penalty = cov_inv.trace();
        let noise_gradient = self.noise * (data_fit - complexity_penalty);
        results.push(noise_gradient);*/

        (scale, results)
    }

    /// Fit parameters using a gradient descent algorithm.
    /// Additionally, at each step, the kernel and noise are rescaled using the optimal magnitude.
    ///
    /// Runs for a maximum of `max_iter` iterations (100 is a good default value).
    /// Stops prematurely if all the components of the gradient go below `convergence_fraction` time the value of their respectively parameter (0.05 is a good default value).
    /// Stops prematurely if the runtime exceeds `max_time`.
    pub(super) fn scaled_optimize_parameters(
        &mut self,
        max_iter: usize,
        convergence_fraction: f64,
        max_time: Duration,
    ) {
        // use the ADAM gradient descent algorithm
        // see [optimizing-gradient-descent](https://ruder.io/optimizing-gradient-descent/)
        // for a good point on current gradient descent algorithms

        // Constant parameters.
        let beta1 = 0.9;
        let beta2 = 0.999;
        let epsilon = 1e-8;
        let learning_rate = 0.1;

        let mut parameters: Vec<_> = self
            .kernel
            .get_parameters()
            .iter()
            .map(|&p| if p == 0. { epsilon } else { p }) // Insures no parameter is 0 (which would block the algorithm).
            .collect();
        let mut mean_grad = vec![0.; parameters.len()];
        let mut var_grad = vec![0.; parameters.len()];

        let time_start = Utc::now();
        for i in 1..=max_iter {
            let (scale, gradients) = self.scaled_gradient_marginal_likelihood();

            let mut had_significant_progress = false;
            for p in 0..parameters.len() {
                mean_grad[p] = beta1 * mean_grad[p] + (1. - beta1) * gradients[p];
                var_grad[p] = beta2 * var_grad[p] + (1. - beta2) * gradients[p].powi(2);
                let bias_corrected_mean = mean_grad[p] / (1. - beta1.powi(i as i32));
                let bias_corrected_variance = var_grad[p] / (1. - beta2.powi(i as i32));
                let delta = learning_rate * bias_corrected_mean / (bias_corrected_variance.sqrt() + epsilon);
                had_significant_progress |= delta.abs() > convergence_fraction;
                parameters[p] *= 1. + delta;
            }

            // Set parameters.
            self.kernel.set_parameters(&parameters);
            self.kernel.rescale(scale);
            self.noise *= scale;
            parameters = self.kernel.get_parameters(); // Get parameters back as they have been rescaled.

            // Fits model.
            self.covmat_cholesky = make_cholesky_cov_matrix(
                &self.training_inputs.as_matrix(),
                &self.kernel,
                self.noise,
                self.cholesky_epsilon,
            );

            if (!had_significant_progress) || (Utc::now().signed_duration_since(time_start) > max_time) {
                //println!("Iterations:{}", i);
                break;
            };
        }

        /*println!("Scaled fit done. likelihood:{} parameters:{:?} noise:{:e}",
        self.likelihood(),
        parameters,
        self.noise);*/
    }
}
