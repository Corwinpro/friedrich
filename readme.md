# Gaussian Process

This libarie implements [Gaussian Process Regression](https://en.wikipedia.org/wiki/Gaussian_process) in Rust.
Our goal is to provide a building block for other algorithms (such as [Bayesian Optimization](https://en.wikipedia.org/wiki/Bayesian_optimization)).

Gaussian process have both the ability to extract a lot of information from their training data and to return a prediction and an uncertainty on their prediction.
Furthermore, they can handle non-linear phenomenons, take uncertainty on the inputs into account and encode a prior on the output.

All of those properties make them an algorithm of choice to perform regression when data is scarce or when having uncertainty bars on the ouput is a desirable property.

However, the `o(n^3)` complexity of the algorithm makes the classical implementation unsuitable for large datasets.

## Usage

training

prediction

sampling

adding points

## TODO

- Clean-up the documentation.
- Add better algorithms to fit kernel parameters.
- Add ndarray support behind a feature flag.
- Add simple kernel regression.

*Do not hesitate to send pull request or ask for features.*
