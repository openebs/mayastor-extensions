Feature: Upgrade

  Background:
    Given the latest mayastor helm chart is installed
    Then all io-engine nodes shall be listed by kubectl-mayastor

  Scenario: Upgrading to the local chart as v-next
    When a kubectl mayastor upgrade command is issued
    Then eventually the installed chart should be upgraded to the kubectl mayastor plugin's version
