use nalgebra::*;
use crate::conversion::{MatrixSlice, VectorSlice, InputRef, OutputRef};

//-----------------------------------------------------------------------------
// MATRIX

/// a matrix that can grow to add additional rows efficiently
pub struct EMatrix
{
   data: DMatrix<f64>,
   nrows: usize
}

impl EMatrix
{
   pub fn new(data: DMatrix<f64>) -> Self
   {
      let nrows = data.nrows();
      EMatrix { data, nrows }
   }

   /// add rows to the matrix
   pub fn add_rows(&mut self, rows: MatrixSlice)
   {
      // if we do not have enough rows, we grow the underlying matrix
      let capacity = self.data.nrows();
      let required_size = self.nrows + rows.nrows();
      if required_size > capacity
      {
         // compute new capacity
         let growed_capacity = (3 * capacity) / 2; // capacity increased by a factor 1.5
         let new_capacity = std::cmp::max(required_size, growed_capacity);
         // builds new matrix with more rows
         let mut new_data = DMatrix::from_element(new_capacity, self.data.ncols(), std::f64::NAN);
         new_data.index_mut((..self.nrows, ..)).copy_from(&self.data);
         self.data = new_data;
      }

      // add rows below data
      self.data.index_mut((self.nrows.., ..)).copy_from(&rows);
      self.nrows += rows.nrows();
   }
}

/// converts a ref to an extendable matrix to a slice that points to the actual data
impl InputRef for EMatrix
{
   /// converts a ref to an extendable matrix to a slice that points to the actual data
   fn to_mslice(&self) -> MatrixSlice
   {
      self.data.index((..self.nrows, ..))
   }
}

//-----------------------------------------------------------------------------
// VECTOR

/// a vector that can grow to add additional entries efficiently
pub struct EVector
{
   data: DVector<f64>,
   nrows: usize
}

impl EVector
{
   pub fn new(data: DVector<f64>) -> Self
   {
      let nrows = data.nrows();
      EVector { data, nrows }
   }

   /// add rows to the vector
   pub fn add_rows(&mut self, rows: VectorSlice)
   {
      // if we do not have enough rows, we grow the underlying vector
      let capacity = self.data.nrows();
      let required_size = self.nrows + rows.nrows();
      if required_size > capacity
      {
         // compute new capacity
         let growed_capacity = (3 * capacity) / 2; // capacity increased by a factor 1.5
         let new_capacity = std::cmp::max(required_size, growed_capacity);
         // builds new matrix with more rows
         let mut new_data = DVector::from_element(new_capacity, std::f64::NAN);
         new_data.index_mut((..self.nrows, ..)).copy_from(&self.data);
         self.data = new_data;
      }

      // add rows below data
      self.data.index_mut((self.nrows.., ..)).copy_from(&rows);
      self.nrows += rows.nrows();
   }

   /// assigns new content to the vector
   /// the new vector must be of the same size as the old vector
   pub fn assign(&mut self, rows: DVector<f64>)
   {
      assert_eq!(rows.nrows(), self.nrows);
      self.data.index_mut((..rows.nrows(), ..)).copy_from(&rows);
   }
}

/// converts a ref to an extendable vector to a slice that points to the actual data
impl OutputRef for EVector
{
   /// converts a ref to an extendable vector to a slice that points to the actual data
   fn to_vslice(&self) -> VectorSlice
   {
      self.data.index((..self.nrows, ..))
   }
}