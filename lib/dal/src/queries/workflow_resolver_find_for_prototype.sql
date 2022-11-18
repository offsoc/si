SELECT DISTINCT ON (workflow_resolvers.id) workflow_resolvers.id,
                                                workflow_resolvers.visibility_change_set_pk,

                                                workflow_resolvers.component_id,
                                                workflow_resolvers.schema_id,
                                                workflow_resolvers.schema_variant_id,
                                                row_to_json(workflow_resolvers.*) as object
FROM workflow_resolvers_v1($1, $2) as workflow_resolvers
WHERE workflow_resolvers.workflow_prototype_id = $3
  AND (workflow_resolvers.component_id = $4
       OR workflow_resolvers.schema_id = $5
       OR workflow_resolvers.schema_variant_id = $6)
ORDER BY workflow_resolvers.id,
         visibility_change_set_pk DESC,
         component_id DESC,
         schema_variant_id DESC,
         schema_id DESC;

