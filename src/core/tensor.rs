//! N-dimensional Tensor type for multi-dimensional array operations.
//!
//! Provides a generic dense tensor with shape tracking, indexing,
//! arithmetic operations, and reshaping capabilities.

use crate::core::types::Scalar;

/// Shape descriptor for an N-dimensional tensor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TensorDims(Vec<usize>);

impl TensorDims {
    pub fn new(shape: Vec<usize>) -> Self {
        assert!(
            !shape.is_empty(),
            "TensorDims must have at least one dimension"
        );
        Self(shape)
    }

    pub fn scalar() -> Self {
        Self(vec![1])
    }

    pub fn vector(n: usize) -> Self {
        Self(vec![n])
    }

    pub fn matrix(rows: usize, cols: usize) -> Self {
        Self(vec![rows, cols])
    }

    pub fn dims(&self) -> &[usize] {
        &self.0
    }

    pub fn ndim(&self) -> usize {
        self.0.len()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn is_scalar(&self) -> bool {
        self.0.len() == 1 && self.0[0] == 1
    }

    pub fn is_vector(&self) -> bool {
        self.0.len() == 1 && self.0[0] > 1
    }

    pub fn is_matrix(&self) -> bool {
        self.0.len() == 2
    }

    /// Total number of elements.
    pub fn flat_size(&self) -> usize {
        self.0.iter().product()
    }

    /// Compute strides for each dimension (row-major).
    pub fn strides(&self) -> Vec<usize> {
        let mut s = Vec::with_capacity(self.0.len());
        let mut stride = 1;
        for dim in self.0.iter().rev() {
            s.push(stride);
            stride *= dim;
        }
        s.reverse();
        s
    }

    /// Convert multi-dimensional indices to flat index.
    pub fn flat_index(&self, indices: &[usize]) -> Option<usize> {
        if indices.len() != self.0.len() {
            return None;
        }
        let strides = self.strides();
        let mut idx = 0;
        for (i, &dim) in self.0.iter().enumerate() {
            if indices[i] >= dim {
                return None;
            }
            idx += indices[i] * strides[i];
        }
        Some(idx)
    }
}

/// An N-dimensional dense tensor of `Scalar` values.
#[derive(Debug, Clone, PartialEq)]
pub struct Tensor {
    dims: TensorDims,
    data: Vec<Scalar>,
}

impl Tensor {
    /// Create a new tensor with the given shape, initialized to zero.
    pub fn new(dims: TensorDims) -> Self {
        let size = dims.flat_size();
        Self {
            data: vec![0.0; size],
            dims,
        }
    }

    /// Create a tensor from a flat vector with the given shape.
    pub fn from_vec(dims: TensorDims, data: Vec<Scalar>) -> Self {
        assert_eq!(
            data.len(),
            dims.flat_size(),
            "data length must match tensor dimensions"
        );
        Self { dims, data }
    }

    /// Create a tensor filled with a constant value.
    pub fn fill(dims: TensorDims, value: Scalar) -> Self {
        let size = dims.flat_size();
        Self {
            data: vec![value; size],
            dims,
        }
    }

    /// Access an element by multi-dimensional indices.
    pub fn get(&self, indices: &[usize]) -> Option<Scalar> {
        self.dims.flat_index(indices).map(|i| self.data[i])
    }

    /// Set an element at multi-dimensional indices.
    pub fn set(&mut self, indices: &[usize], value: Scalar) -> Option<()> {
        let i = self.dims.flat_index(indices)?;
        self.data[i] = value;
        Some(())
    }

    /// Reshape the tensor to new dimensions (must have same flat size).
    pub fn reshape(&self, new_dims: TensorDims) -> Option<Self> {
        if new_dims.flat_size() != self.dims.flat_size() {
            return None;
        }
        Some(Self {
            data: self.data.clone(),
            dims: new_dims,
        })
    }

    /// Apply a function element-wise.
    pub fn map(&self, f: impl Fn(Scalar) -> Scalar) -> Self {
        Self {
            data: self.data.iter().map(|&v| f(v)).collect(),
            dims: self.dims.clone(),
        }
    }

    /// Add two tensors element-wise (broadcast to common shape).
    pub fn add(&self, other: &Tensor) -> Option<Self> {
        if self.dims != other.dims {
            return None;
        }
        let data: Vec<Scalar> = self
            .data
            .iter()
            .zip(other.data.iter())
            .map(|(a, b)| a + b)
            .collect();
        Some(Self {
            data,
            dims: self.dims.clone(),
        })
    }

    /// Multiply two tensors element-wise.
    pub fn mul(&self, other: &Tensor) -> Option<Self> {
        if self.dims != other.dims {
            return None;
        }
        let data: Vec<Scalar> = self
            .data
            .iter()
            .zip(other.data.iter())
            .map(|(a, b)| a * b)
            .collect();
        Some(Self {
            data,
            dims: self.dims.clone(),
        })
    }

    /// Transpose a 2D tensor (matrix).
    pub fn transpose(&self) -> Option<Self> {
        if !self.dims.is_matrix() {
            return None;
        }
        let rows = self.dims.dims()[0];
        let cols = self.dims.dims()[1];
        let mut new_data = vec![0.0; rows * cols];
        for i in 0..rows {
            for j in 0..cols {
                new_data[j * rows + i] = self.data[i * cols + j];
            }
        }
        Some(Self {
            data: new_data,
            dims: TensorDims::matrix(cols, rows),
        })
    }

    pub fn shape(&self) -> &TensorDims {
        &self.dims
    }

    pub fn data(&self) -> &[Scalar] {
        &self.data
    }

    pub fn into_data(self) -> Vec<Scalar> {
        self.data
    }

    /// Convert to a scalar value (panics if not scalar-shaped).
    pub fn to_scalar(&self) -> Option<Scalar> {
        if self.dims.is_scalar() {
            Some(self.data[0])
        } else {
            None
        }
    }

    /// Convert to a vector (panics if not 1D).
    pub fn to_vec(&self) -> Option<Vec<Scalar>> {
        if self.dims.is_vector() || self.dims.is_scalar() {
            Some(self.data.clone())
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tensor_scalar() {
        let t = Tensor::from_vec(TensorDims::scalar(), vec![42.0]);
        assert_eq!(t.to_scalar(), Some(42.0));
    }

    #[test]
    fn test_tensor_vector() {
        let t = Tensor::from_vec(TensorDims::vector(3), vec![1.0, 2.0, 3.0]);
        assert_eq!(t.get(&[1]), Some(2.0));
        assert_eq!(t.to_vec(), Some(vec![1.0, 2.0, 3.0]));
    }

    #[test]
    fn test_tensor_matrix() {
        let mut t = Tensor::from_vec(TensorDims::matrix(2, 3), vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        assert_eq!(t.get(&[1, 1]), Some(5.0));
        t.set(&[0, 0], 10.0).unwrap();
        assert_eq!(t.get(&[0, 0]), Some(10.0));
    }

    #[test]
    fn test_transpose() {
        let t = Tensor::from_vec(TensorDims::matrix(2, 3), vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        let tr = t.transpose().unwrap();
        assert_eq!(tr.shape().dims(), &[3, 2]);
        assert_eq!(tr.get(&[0, 0]), Some(1.0));
        assert_eq!(tr.get(&[1, 0]), Some(2.0));
    }

    #[test]
    fn test_reshape() {
        let t = Tensor::from_vec(TensorDims::new(vec![6]), vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        let r = t.reshape(TensorDims::matrix(2, 3)).unwrap();
        assert_eq!(r.shape().dims(), &[2, 3]);
    }

    #[test]
    fn test_elementwise_add() {
        let a = Tensor::from_vec(TensorDims::vector(3), vec![1.0, 2.0, 3.0]);
        let b = Tensor::from_vec(TensorDims::vector(3), vec![4.0, 5.0, 6.0]);
        let c = a.add(&b).unwrap();
        assert_eq!(c.to_vec(), Some(vec![5.0, 7.0, 9.0]));
    }

    #[test]
    fn test_tensor_dims_strides() {
        let d = TensorDims::matrix(3, 4);
        let strides = d.strides();
        assert_eq!(strides, vec![4, 1]);
        assert_eq!(d.flat_size(), 12);
        assert_eq!(d.flat_index(&[2, 3]), Some(11));
        assert_eq!(d.flat_index(&[3, 0]), None); // out of bounds
    }
}
