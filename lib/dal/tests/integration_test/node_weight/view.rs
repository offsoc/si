use dal::{
    diagram::{geometry::Geometry, view::View},
    Component, DalContext,
};
use dal_test::{
    expected::{self, generate_fake_name, ExpectView},
    helpers::create_component_for_default_schema_name,
    test,
};
use pretty_assertions_sorted::assert_eq;

#[test]
async fn correct_transforms_remove_view_all_geometries_removed(ctx: &mut DalContext) {
    let default_view_id = View::get_id_for_default(ctx)
        .await
        .expect("Unable to get default ViewId");
    let new_view = ExpectView::create(ctx).await;
    let component = create_component_for_default_schema_name(
        ctx,
        "swifty",
        generate_fake_name(),
        new_view.id(),
    )
    .await
    .expect("Unable to create Component in View");

    expected::apply_change_set_to_base(ctx).await;
    expected::fork_from_head_change_set(ctx).await;

    assert_eq!(
        2,
        View::list(ctx).await.expect("Unable to list Views").len()
    );

    assert_eq!(
        1,
        Geometry::list_by_view_id(ctx, new_view.id())
            .await
            .expect("Unable to list Geometries for View")
            .len(),
    );

    Component::remove(ctx, component.id())
        .await
        .expect("Unable to remove Component");

    let views = View::list(ctx).await.expect("Unable to list Views");
    assert_eq!(2, views.len());

    assert_eq!(
        0,
        Geometry::list_by_view_id(ctx, new_view.id())
            .await
            .expect("Unable to list Geometries for View")
            .len(),
    );

    View::remove(ctx, new_view.id())
        .await
        .expect("Unable to remove view");

    assert_eq!(
        1,
        View::list(ctx).await.expect("Unable to list Views").len()
    );

    expected::apply_change_set_to_base(ctx).await;

    assert_eq!(
        0,
        Geometry::list_by_view_id(ctx, default_view_id)
            .await
            .expect("Unable to list Geometries for default View")
            .len()
    );
    assert_eq!(
        1,
        View::list(ctx).await.expect("Unable to list Views").len()
    );
}

#[test]
async fn correct_transforms_remove_view_already_removed_view(ctx: &mut DalContext) {
    let new_view = ExpectView::create(ctx).await;
    expected::apply_change_set_to_base(ctx).await;

    let change_set_one = expected::fork_from_head_change_set(ctx).await;
    View::remove(ctx, new_view.id())
        .await
        .expect("Unable to remove View in ChangeSet one");

    assert_eq!(
        1,
        View::list(ctx).await.expect("Unable to list Views").len()
    );

    let _change_set_two = expected::fork_from_head_change_set(ctx).await;
    View::remove(ctx, new_view.id())
        .await
        .expect("Unable to remove View from ChangeSet two");

    assert_eq!(
        1,
        View::list(ctx).await.expect("Unable to list Views").len()
    );

    expected::apply_change_set_to_base(ctx).await;
    ctx.update_visibility_and_snapshot_to_visibility(change_set_one.id)
        .await
        .expect("Unable to switch to ChangeSet one");
    expected::apply_change_set_to_base(ctx).await;

    assert_eq!(
        1,
        View::list(ctx).await.expect("Unable to list Views").len()
    );
}

#[test]
async fn correct_transforms_remove_view_not_all_geometries_removed(ctx: &mut DalContext) {
    let new_view = ExpectView::create(ctx).await;
    let views = View::list(ctx).await.expect("Unable to list Views");
    assert_eq!(2, views.len());
    expected::apply_change_set_to_base(ctx).await;

    let view_removal_change_set = expected::fork_from_head_change_set(ctx).await;
    View::remove(ctx, new_view.id())
        .await
        .expect("Unable to remove View in ChangeSet");
    assert_eq!(
        1,
        View::list(ctx)
            .await
            .expect("Unable to list Views in view removal ChangeSet")
            .len(),
    );
    expected::commit_and_update_snapshot_to_visibility(ctx).await;

    expected::fork_from_head_change_set(ctx).await;

    create_component_for_default_schema_name(ctx, "swifty", generate_fake_name(), new_view.id())
        .await
        .expect("Unable to create component");

    expected::apply_change_set_to_base(ctx).await;
    assert_eq!(
        2,
        View::list(ctx)
            .await
            .expect("Unable to list Views in base ChangeSet")
            .len(),
    );
    assert_eq!(
        1,
        Geometry::list_by_view_id(ctx, new_view.id())
            .await
            .expect("Unable to list Geometries in new View")
            .len(),
    );

    ctx.update_visibility_and_snapshot_to_visibility(view_removal_change_set.id)
        .await
        .expect("Unable to switch to view removal ChangeSet");

    assert_eq!(
        1,
        View::list(ctx)
            .await
            .expect("Unable to list Views in view removal ChangeSet")
            .len(),
    );

    expected::apply_change_set_to_base(ctx).await;

    assert_eq!(
        2,
        View::list(ctx)
            .await
            .expect("Unable to list Views in base ChangeSet")
            .len(),
    );
}

#[test]
async fn correct_transforms_remove_view_no_other_views(ctx: &mut DalContext) {
    let new_view = ExpectView::create(ctx).await;
    expected::apply_change_set_to_base(ctx).await;

    assert_eq!(
        2,
        View::list(ctx).await.expect("Unable to list Views").len()
    );
    assert_eq!(
        0,
        Geometry::list_by_view_id(ctx, new_view.id())
            .await
            .expect("Unable to list Geometries in new View")
            .len(),
    );

    let view_removal_change_set = expected::fork_from_head_change_set(ctx).await;
    View::remove(ctx, new_view.id())
        .await
        .expect("Unable to remove View in ChangeSet");
    expected::commit_and_update_snapshot_to_visibility(ctx).await;

    expected::fork_from_head_change_set(ctx).await;
    create_component_for_default_schema_name(ctx, "swifty", generate_fake_name(), new_view.id())
        .await
        .expect("Unable to create Component in new View");
    expected::apply_change_set_to_base(ctx).await;
    assert_eq!(
        1,
        Geometry::list_by_view_id(ctx, new_view.id())
            .await
            .expect("Unable to list Geometries in new View")
            .len(),
    );

    ctx.update_visibility_and_snapshot_to_visibility(view_removal_change_set.id)
        .await
        .expect("Unable to switch to View removal ChangeSet");
    assert_eq!(
        1,
        View::list(ctx)
            .await
            .expect("Unable to list Views in base ChangeSet")
            .len(),
    );
    expected::apply_change_set_to_base(ctx).await;

    assert_eq!(
        2,
        View::list(ctx)
            .await
            .expect("Unable to list Views in base ChangeSet")
            .len(),
    );
    assert_eq!(
        1,
        Geometry::list_by_view_id(ctx, new_view.id())
            .await
            .expect("Unable to list Geometries in new View")
            .len(),
    );
}
