use dal::{
    test::helpers::{create_group, create_user},
    BillingAccountId, DalContext, Group, StandardModel,
};

use crate::dal::test;

#[test]
async fn new(ctx: &mut DalContext<'_, '_, '_>, bid: BillingAccountId) {
    ctx.update_to_billing_account_tenancies(bid);

    let _group = Group::new(ctx, "funky").await.expect("cannot create group");
}

#[test]
async fn add_user(ctx: &mut DalContext<'_, '_, '_>, bid: BillingAccountId) {
    ctx.update_to_billing_account_tenancies(bid);

    let group = create_group(ctx).await;
    let user_one = create_user(ctx).await;
    let user_two = create_user(ctx).await;

    group
        .add_user(ctx, user_one.id())
        .await
        .expect("cannot add user");
    group
        .add_user(ctx, user_two.id())
        .await
        .expect("cannot add user");
}

#[test]
async fn remove_user(ctx: &mut DalContext<'_, '_, '_>, bid: BillingAccountId) {
    ctx.update_to_billing_account_tenancies(bid);

    let group = create_group(ctx).await;
    let user_one = create_user(ctx).await;
    let user_two = create_user(ctx).await;

    group
        .add_user(ctx, user_one.id())
        .await
        .expect("cannot add user");
    group
        .add_user(ctx, user_two.id())
        .await
        .expect("cannot add user");

    group
        .remove_user(ctx, user_one.id())
        .await
        .expect("cannot remove user");
    group
        .remove_user(ctx, user_two.id())
        .await
        .expect("cannot remove user");
}

#[test]
async fn users(ctx: &mut DalContext<'_, '_, '_>, bid: BillingAccountId) {
    ctx.update_to_billing_account_tenancies(bid);

    let group = create_group(ctx).await;
    let user_one = create_user(ctx).await;
    let user_two = create_user(ctx).await;

    group
        .add_user(ctx, user_one.id())
        .await
        .expect("cannot add user");
    group
        .add_user(ctx, user_two.id())
        .await
        .expect("cannot add user");

    let all_users = group.users(ctx).await.expect("cannot list users for group");
    assert_eq!(
        all_users,
        vec![user_one.clone(), user_two.clone()],
        "all associated users in the list"
    );

    group
        .remove_user(ctx, user_one.id())
        .await
        .expect("cannot remove user");

    let some_users = group.users(ctx).await.expect("cannot list users for group");
    assert_eq!(
        some_users,
        vec![user_two.clone()],
        "only one associated user in the list"
    );
}
