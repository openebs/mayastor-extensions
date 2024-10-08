import git


def root_dir():
    file_path = os.path.abspath(__file__)
    return file_path.split("tests/bdd")[0]


def latest_tag():
    repo = git.Repo(root_dir())
    tags = sorted(repo.tags, key=lambda t: t.commit.committed_datetime)
    return tags[-1]
