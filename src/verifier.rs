use ark_ec::{AffineCurve, PairingEngine, ProjectiveCurve};
use ark_ff::{PrimeField, to_bytes};

use super::{PreparedVerifyingKey, Proof, VerifyingKey};

use ark_relations::r1cs::{Result as R1CSResult, SynthesisError};

use core::ops::{AddAssign, Neg};

use blake2::{Blake2b, Digest};

/// Prepare the verifying key `vk` for use in proof verification.
pub fn prepare_verifying_key<E: PairingEngine>(vk: &VerifyingKey<E>) -> PreparedVerifyingKey<E> {
    PreparedVerifyingKey {
        vk: vk.clone(),
        gamma_g2_neg_pc: vk.gamma_g2.neg().into(),
    }
}

/// Prepare proof inputs for use with [`verify_proof_with_prepared_inputs`], wrt the prepared
/// verification key `pvk` and instance public inputs.
pub fn prepare_inputs<E: PairingEngine>(
    pvk: &PreparedVerifyingKey<E>,
    public_inputs: &[E::Fr],
) -> R1CSResult<E::G1Projective> {
    if (public_inputs.len() + 1) != pvk.vk.gamma_abc_g1.len() {
        return Err(SynthesisError::MalformedVerifyingKey);
    }

    let mut g_ic = pvk.vk.gamma_abc_g1[0].into_projective();
    for (i, b) in public_inputs.iter().zip(pvk.vk.gamma_abc_g1.iter().skip(1)) {
        g_ic.add_assign(&b.mul(i.into_repr()));
    }

    Ok(g_ic)
}

/// Verify a proof `proof` against the prepared verification key `pvk` and prepared public
/// inputs. This should be preferred over [`verify_proof`] if the instance's public inputs are
/// known in advance.
pub fn verify_proof_with_prepared_inputs<E: PairingEngine>(
    pvk: &PreparedVerifyingKey<E>,
    proof: &Proof<E>,
    prepared_inputs: &E::G1Projective,
) -> R1CSResult<bool> {
    let qap = E::miller_loop(
        [
            (proof.a.into(), proof.b.into()),
            (
                prepared_inputs.into_affine().into(),
                pvk.gamma_g2_neg_pc.clone(),
            ),
            (proof.c.into(), proof.delta_prime.neg().into()),
        ]
        .iter(),
    );

    let test = E::final_exponentiation(&qap).ok_or(SynthesisError::UnexpectedIdentity)?;


    let hash = Blake2b::new()
    .chain(to_bytes!(&proof.a).unwrap())
    .chain(to_bytes!(&proof.b).unwrap())
    .chain(to_bytes!(&proof.c).unwrap())
    .chain(to_bytes!(&proof.delta_prime).unwrap());
    let mut output = [0u8; 64];
    output.copy_from_slice(&hash.finalize());

    let m_fr = E::Fr::from_le_bytes_mod_order(&output);
    //println!("m_fr verifier {0}", m_fr);
    let mut delta_prime_delta_m = pvk.vk.delta_g2.mul(m_fr);
    delta_prime_delta_m.add_assign_mixed(&proof.delta_prime);

    let test2 = E::pairing(proof.d, delta_prime_delta_m.into_affine());

    Ok((test == pvk.vk.alpha_g1_beta_g2) && (test2 == pvk.vk.zt_gt))
}

/// Verify a proof `proof` against the prepared verification key `pvk`,
/// with respect to the instance `public_inputs`.
pub fn verify_proof<E: PairingEngine>(
    pvk: &PreparedVerifyingKey<E>,
    proof: &Proof<E>,
    public_inputs: &[E::Fr],
) -> R1CSResult<bool> {
    let prepared_inputs = prepare_inputs(pvk, public_inputs)?;
    verify_proof_with_prepared_inputs(pvk, proof, &prepared_inputs)
}
