use arrow_array::{
    ArrayRef, FixedSizeListArray, RecordBatch, RecordBatchIterator, StringArray, types::Float32Type,
};
use arrow_schema::{DataType, Field, Schema};
use lancedb::{Connection, Result as LanceDbResult, Table};
use rig::{
    Embed, OneOrMany,
    embeddings::Embedding,
    vector_store::{InsertDocuments, VectorStoreError},
};
use serde::Serialize;
use std::sync::Arc;

use crate::reader::ChunkedDocument;

pub struct LanceDbDocumentStore {
    table: Table,
}

impl LanceDbDocumentStore {
    pub fn new(table: Table) -> Self {
        Self { table }
    }

    pub async fn create_table(
        db: &Connection,
        table_name: &str,
        dims: usize,
    ) -> LanceDbResult<Table> {
        db.create_empty_table(table_name, Self::schema(dims))
            .execute()
            .await
    }

    pub async fn open_table(db: &Connection, table_name: &str) -> LanceDbResult<Table> {
        db.open_table(table_name).execute().await
    }

    pub async fn is_exsist_table(db: &Connection, table_name: &str) -> LanceDbResult<bool> {
        let table_names = db.table_names().execute().await?;
        Ok(table_names.iter().any(|name| name == table_name))
    }

    fn schema(dims: usize) -> Arc<Schema> {
        Arc::new(Schema::new(vec![
            Field::new("path", DataType::Utf8, false),
            Field::new("chapter", DataType::Utf8, false),
            Field::new("doc", DataType::Utf8, false),
            Field::new(
                "vector",
                DataType::FixedSizeList(
                    Arc::new(Field::new("item", DataType::Float32, true)),
                    dims as i32,
                ),
                false,
            ),
        ]))
    }
}

impl InsertDocuments for LanceDbDocumentStore {
    fn insert_documents<Doc: Serialize + Embed + Send>(
        &self,
        documents: Vec<(Doc, OneOrMany<Embedding>)>,
    ) -> impl std::future::Future<Output = Result<(), VectorStoreError>> + Send {
        let table = self.table.clone();

        async move {
            if documents.is_empty() {
                return Ok(());
            }

            let dims = documents
                .first()
                .map(|(_, e)| e.first().vec.len())
                .ok_or_else(|| {
                    VectorStoreError::BuilderError("Empty embedding result".to_string())
                })?;

            let mut path_values = Vec::with_capacity(documents.len());
            let mut chapter_values = Vec::with_capacity(documents.len());
            let mut doc_values = Vec::with_capacity(documents.len());
            let mut vector_values = Vec::with_capacity(documents.len());

            for (doc, embedded) in documents {
                let doc_value = serde_json::to_value(doc).map_err(VectorStoreError::JsonError)?;
                let str_doc: ChunkedDocument = serde_json::from_value(doc_value.clone())
                    .map_err(VectorStoreError::JsonError)?;
                path_values.push(str_doc.path);
                chapter_values.push(str_doc.chapter);
                doc_values.push(str_doc.doc);

                let vector = embedded.first().vec;
                if vector.len() != dims {
                    return Err(VectorStoreError::BuilderError(format!(
                        "Embedding dimension mismatch. expected={dims}, actual={}",
                        vector.len()
                    )));
                }
                vector_values.push(Some(
                    vector
                        .into_iter()
                        .map(|v| Some(v as f32))
                        .collect::<Vec<_>>(),
                ));
            }

            let schema = Self::schema(dims);

            let path_array = StringArray::from(path_values);
            let chapter_array = StringArray::from(chapter_values);
            let document_array = StringArray::from(doc_values);
            let embedding_array = FixedSizeListArray::from_iter_primitive::<Float32Type, _, _>(
                vector_values,
                dims as i32,
            );

            let batch = RecordBatch::try_new(
                schema.clone(),
                vec![
                    Arc::new(path_array) as ArrayRef,
                    Arc::new(chapter_array) as ArrayRef,
                    Arc::new(document_array) as ArrayRef,
                    Arc::new(embedding_array) as ArrayRef,
                ],
            )
            .map_err(|e| VectorStoreError::DatastoreError(Box::new(e)))?;

            table
                .add(RecordBatchIterator::new(vec![Ok(batch)], schema))
                .execute()
                .await
                .map_err(|e| VectorStoreError::DatastoreError(Box::new(e)))?;

            Ok(())
        }
    }
}
