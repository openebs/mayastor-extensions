from time import sleep


# Accepts a predicate and retry details and returns true if predicate returns a True
# eventually within the retry duration, else returns a False.
def retry_predicate(predicate, retry_count: int, retry_interval_in_seconds: int) -> bool:
    yes = False
    for _ in range(retry_count):
        yes = predicate()
        if yes:
            break
        sleep(retry_interval_in_seconds)

    return yes
