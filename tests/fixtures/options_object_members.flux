import "strings"
import "csv"

cal = 10
env = "prod01-us-west-2"

cool = (a) => a + 1

option task = {
  name: "foo",        // Name is required.
  every: 1h,          // Task should be run at this interval.
  delay: 10m,         // Delay scheduling this task by this duration.
  cron: "0 2 * * *",  // Cron is a more sophisticated way to schedule. 'every' and 'cron' are mutually exclusive.
  retry: 5,           // Number of times to retry a failed query.
}

task.

ab = 10
