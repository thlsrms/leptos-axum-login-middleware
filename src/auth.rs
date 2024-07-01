use axum::async_trait;
use axum_login::{AuthUser, AuthnBackend, AuthzBackend};
use std::collections::HashMap;

#[repr(u8)]
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum Role {
    User = 100,
    Admin = 255,
}

impl From<Role> for u8 {
    fn from(r: Role) -> u8 {
        r as u8
    }
}

#[derive(Debug, Clone)]
pub struct UserId(pub String);

#[derive(Clone, Debug)]
pub struct User {
    id: UserId,
    pw_hash: Vec<u8>,
    roles: Vec<u8>,
}

impl AuthUser for User {
    type Id = String;

    fn id(&self) -> Self::Id {
        self.id.0.clone()
    }

    fn session_auth_hash(&self) -> &[u8] {
        &self.pw_hash
    }
}

pub type AuthSession = axum_login::AuthSession<Backend>;

#[derive(Clone, Default, Debug)]
pub struct Backend {
    users: HashMap<String, User>,
}

impl Backend {
    pub fn register_user(
        &mut self,
        new_user_id: &str,
        roles: &[u8],
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.users.insert(
            new_user_id.into(),
            User {
                id: UserId(new_user_id.into()),
                pw_hash: new_user_id.into(),
                roles: roles.to_vec(),
            },
        );
        Ok(())
    }
}

#[async_trait]
impl AuthnBackend for Backend {
    type User = User;
    type Credentials = UserId;
    type Error = std::convert::Infallible;

    async fn authenticate(
        &self,
        UserId(id): Self::Credentials,
    ) -> Result<Option<Self::User>, Self::Error> {
        Ok(self.get_user(&id).await.expect("Failed to get_user"))
    }

    async fn get_user(
        &self,
        user_id: &axum_login::UserId<Self>,
    ) -> Result<Option<Self::User>, Self::Error> {
        Ok(self.users.get(user_id).cloned())
    }
}

#[async_trait]
impl AuthzBackend for Backend {
    type Permission = u8;

    async fn get_user_permissions(
        &self,
        user: &Self::User,
    ) -> Result<std::collections::HashSet<Self::Permission>, Self::Error> {
        let mut user_roles = std::collections::HashSet::<Self::Permission>::new();
        user_roles.extend(user.roles.to_vec());
        Ok(user_roles)
    }

    async fn get_group_permissions(
        &self,
        _user: &Self::User,
    ) -> Result<std::collections::HashSet<Self::Permission>, Self::Error> {
        Ok(std::collections::HashSet::new())
    }

    async fn get_all_permissions(
        &self,
        user: &Self::User,
    ) -> Result<std::collections::HashSet<Self::Permission>, Self::Error> {
        let mut all_perms = std::collections::HashSet::new();
        all_perms.extend(self.get_user_permissions(user).await?);
        all_perms.extend(self.get_group_permissions(user).await?);
        Ok(all_perms)
    }

    async fn has_perm(
        &self,
        user: &Self::User,
        perm: Self::Permission,
    ) -> Result<bool, Self::Error> {
        let all_perms = self.get_all_permissions(user).await?;
        Ok(all_perms.contains(&perm) || *all_perms.iter().max().unwrap() > perm)
    }
}
