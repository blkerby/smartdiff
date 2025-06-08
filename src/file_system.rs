use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

// Trait to abstract over whether we are using the local file system (for working copy)
// or git tree (for comparison branch)
pub trait FileSystem {
    fn load(&self, path: &Path) -> Result<Vec<u8>>;
}

pub struct GitTreeFileSystem<'a> {
    pub repo: &'a git2::Repository,
    pub tree: git2::Tree<'a>,
}

fn get_components(path: &Path) -> Result<Vec<String>> {
    let mut out: Vec<String> = vec![];
    for c in path.components().rev() {
        let name = c.as_os_str().to_str().context(format!(
            "Error converting path component to string: {:?}",
            c
        ))?;
        out.push(name.to_string());
    }
    Ok(out)
}

impl<'a> FileSystem for GitTreeFileSystem<'a> {
    fn load(&self, path: &Path) -> Result<Vec<u8>> {
        // We have to manually walk the git tree in order to resolve
        // symbolic links along the way, because git2 doesn't do it.
        let mut obj = self.tree.as_object().clone();
        let mut components: Vec<String> = get_components(path)?;
        let mut parents: Vec<git2::Tree<'a>> = vec![];
        let mut symlink_limit = 40;
        while let Some(name) = components.pop() {
            if name == ".." {
                match parents.pop() {
                    None => {
                        bail!("Invalid reference to parent directory outside of repo");
                    }
                    Some(p) => {
                        obj = p.as_object().clone();
                    }
                }
            } else {
                let tree = obj
                    .as_tree()
                    .context(format!("Parent of component '{}' is not a tree", name,))?;
                let entry = tree
                    .get_name(&name)
                    .context(format!("Error getting component '{}'", name,))?;
                let new_obj = entry
                    .to_object(&self.repo)
                    .context("Error calling to_object")?;
                if entry.filemode() == 0xA000 {
                    // Symbolic link
                    if symlink_limit == 0 {
                        bail!("Symlink limit reached (possibly a cyclic reference)");
                    }
                    symlink_limit -= 1;
                    let blob = new_obj
                        .as_blob()
                        .context("Symbolic link content is not a blob")?;
                    let content = String::from_utf8(blob.content().to_vec())?;
                    let new_path = PathBuf::from(content);
                    components.extend(get_components(&new_path)?);
                } else {
                    parents.push(tree.clone());
                    drop(entry);
                    obj = new_obj;
                }
            }
        }
        let blob = obj
            .as_blob()
            .context(format!("Object exists but is not a blob"))?;
        Ok(blob.content().to_vec())
    }
}

pub struct LocalFileSystem {}

impl FileSystem for LocalFileSystem {
    fn load(&self, path: &Path) -> Result<Vec<u8>> {
        Ok(std::fs::read(path)?)
    }
}
