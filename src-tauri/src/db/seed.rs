/// Technique seeds for the technique_registry table.
/// Each entry: (problem_class, technique_family, description)
pub const TECHNIQUE_SEEDS: &[(&str, &str, &str)] = &[
    // ── Functional Equations ──
    (
        "functional_equation",
        "substitution_special_values",
        "Substitute x=0, y=0, x=y, x=-y to extract initial constraints",
    ),
    (
        "functional_equation",
        "injectivity_surjectivity",
        "Prove or exploit injectivity/surjectivity from the equation",
    ),
    (
        "functional_equation",
        "cauchy_equation",
        "Reduce to Cauchy equation f(x+y)=f(x)+f(y) and classify solutions",
    ),
    (
        "functional_equation",
        "multiplicative_reduction",
        "Transform to multiplicative Cauchy g(xy)=g(x)g(y)",
    ),
    (
        "functional_equation",
        "exponential_construction",
        "Try f(x) = a^{g(x)} for suitable g and base a",
    ),
    (
        "functional_equation",
        "power_function",
        "Test f(x) = x^k for rational k, verify consistency",
    ),
    (
        "functional_equation",
        "polynomial_bounding",
        "Bound degree by comparing growth rates on both sides",
    ),
    (
        "functional_equation",
        "fixed_point_iteration",
        "Find fixed points, iterate the equation to propagate constraints",
    ),
    (
        "functional_equation",
        "monotonicity_argument",
        "Establish monotonicity from the equation, use to force linearity",
    ),
    (
        "functional_equation",
        "involution_detection",
        "Check if f(f(x)) = x or f(f(x)) = f(x) (idempotent)",
    ),
    (
        "functional_equation",
        "regularity_bootstrapping",
        "Show measurable → continuous → differentiable → polynomial",
    ),
    (
        "functional_equation",
        "piecewise_construction",
        "Construct piecewise solutions respecting domain constraints",
    ),
    (
        "functional_equation",
        "logarithmic_substitution",
        "Substitute f(x) = log(g(x)) or x = e^t to simplify",
    ),
    // ── Number Theory ──
    (
        "number_theory",
        "modular_arithmetic",
        "Work modulo small primes to constrain residues",
    ),
    (
        "number_theory",
        "p_adic_valuation",
        "Analyze p-adic valuations v_p on both sides of equations",
    ),
    (
        "number_theory",
        "lifting_the_exponent",
        "Apply LTE lemma for v_p(a^n - b^n) or v_p(a^n + b^n)",
    ),
    (
        "number_theory",
        "order_and_primitive_roots",
        "Use multiplicative orders and primitive roots modulo p",
    ),
    (
        "number_theory",
        "quadratic_reciprocity",
        "Apply QR law and Legendre/Jacobi symbols",
    ),
    (
        "number_theory",
        "vieta_jumping",
        "Root-flipping / Vieta jumping on Diophantine equations",
    ),
    (
        "number_theory",
        "infinite_descent",
        "Apply Fermat's method of infinite descent",
    ),
    (
        "number_theory",
        "chinese_remainder",
        "CRT to combine modular constraints",
    ),
    (
        "number_theory",
        "size_bounding",
        "Bound variables to finite range, then enumerate",
    ),
    (
        "number_theory",
        "divisibility_cascade",
        "Chain divisibility relations a|b|c to constrain solutions",
    ),
    (
        "number_theory",
        "zsygmondy_theorem",
        "Apply Zsygmondy/Bang theorem for primitive prime divisors",
    ),
    (
        "number_theory",
        "algebraic_number_theory",
        "Factor in Z[i], Z[ω], or other rings of integers",
    ),
    // ── Algebra (Inequalities & Polynomials) ──
    (
        "algebra",
        "am_gm_inequality",
        "Apply AM-GM or weighted power mean inequality",
    ),
    (
        "algebra",
        "cauchy_schwarz",
        "Apply Cauchy-Schwarz or Titu's lemma (Engel form)",
    ),
    (
        "algebra",
        "schur_inequality",
        "Apply Schur's inequality or SOS decomposition",
    ),
    (
        "algebra",
        "sos_decomposition",
        "Express as sum of squares to prove non-negativity",
    ),
    (
        "algebra",
        "substitution_normalization",
        "Normalize variables (e.g. a+b+c=1) to reduce dimension",
    ),
    (
        "algebra",
        "lagrange_multipliers",
        "Constrained optimization via Lagrange multipliers",
    ),
    (
        "algebra",
        "tangent_line_trick",
        "Compare function with its tangent at equality point",
    ),
    (
        "algebra",
        "jensen_convexity",
        "Apply Jensen's inequality for convex/concave functions",
    ),
    (
        "algebra",
        "polynomial_roots",
        "Analyze root structure: Vieta's, discriminant, root bounds",
    ),
    (
        "algebra",
        "symmetric_function_theory",
        "Express in terms of elementary symmetric polynomials",
    ),
    (
        "algebra",
        "rearrangement_inequality",
        "Apply rearrangement or Chebyshev sum inequality",
    ),
    // ── Combinatorics ──
    (
        "combinatorics",
        "pigeonhole_principle",
        "Apply pigeonhole or generalized pigeonhole",
    ),
    (
        "combinatorics",
        "double_counting",
        "Count the same quantity two ways",
    ),
    (
        "combinatorics",
        "bijective_proof",
        "Construct explicit bijection between sets",
    ),
    (
        "combinatorics",
        "generating_functions",
        "Encode sequence as power series, extract coefficients",
    ),
    (
        "combinatorics",
        "inclusion_exclusion",
        "Apply inclusion-exclusion principle",
    ),
    (
        "combinatorics",
        "extremal_principle",
        "Consider maximal/minimal element, derive contradiction",
    ),
    (
        "combinatorics",
        "graph_coloring",
        "Model as graph, apply coloring or Ramsey arguments",
    ),
    (
        "combinatorics",
        "probabilistic_method",
        "Show existence via probability > 0",
    ),
    (
        "combinatorics",
        "invariant_monovariant",
        "Find invariant or monovariant (quantity that only increases/decreases)",
    ),
    (
        "combinatorics",
        "greedy_algorithm",
        "Construct solution greedily, prove optimality",
    ),
    (
        "combinatorics",
        "induction_strong",
        "Strong induction with carefully chosen inductive hypothesis",
    ),
    (
        "combinatorics",
        "tiling_coloring",
        "Tile/color the structure to derive parity or impossibility",
    ),
    (
        "combinatorics",
        "hall_theorem",
        "Apply Hall's marriage theorem for matching existence",
    ),
    // ── Geometry ──
    (
        "geometry",
        "coordinate_bash",
        "Set up coordinates, compute algebraically",
    ),
    (
        "geometry",
        "trigonometric_cevian",
        "Use trig cevian formulas, law of sines/cosines",
    ),
    (
        "geometry",
        "projective_transformation",
        "Apply projective or inversive transformation",
    ),
    (
        "geometry",
        "spiral_similarity",
        "Identify spiral similarities and composition of transformations",
    ),
    (
        "geometry",
        "radical_axes",
        "Use radical axis/center for circle intersection problems",
    ),
    (
        "geometry",
        "angle_chasing",
        "Systematic angle chasing using inscribed angle theorem",
    ),
    (
        "geometry",
        "area_method",
        "Compute ratios via areas, use signed area for collinearity",
    ),
    (
        "geometry",
        "inversion",
        "Apply circular inversion to simplify tangency/concyclicity",
    ),
    (
        "geometry",
        "barycentric_coordinates",
        "Use barycentric coordinates for triangle problems",
    ),
    (
        "geometry",
        "complex_numbers",
        "Map points to complex plane, use rotations and distances",
    ),
    // ── Analysis / Sequences ──
    (
        "analysis",
        "telescoping",
        "Rearrange sum/product as telescoping series",
    ),
    (
        "analysis",
        "squeeze_theorem",
        "Bound sequence from above and below with convergent bounds",
    ),
    (
        "analysis",
        "generating_function_ode",
        "Derive ODE for generating function, solve explicitly",
    ),
    (
        "analysis",
        "characteristic_equation",
        "Solve linear recurrence via characteristic polynomial",
    ),
    (
        "analysis",
        "integral_comparison",
        "Compare sum with integral for asymptotic bounds",
    ),
    (
        "analysis",
        "contraction_mapping",
        "Show iteration is a contraction, apply Banach fixed-point",
    ),
];
