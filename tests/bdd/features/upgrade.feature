Feature: Upgrade

  Background:
    Given a 2-worker node kind kubernetes cluster
    And a v-next chart is prepared
    And the images and plugin are built for v-next
    And the images are loadable from the cluster
    And the latest mayastor helm chart is installed
    And all io-engine nodes shall be listed by kubectl-mayastor

  Scenario: Upgrading to the local chart as v-next
    When a kubectl mayastor upgrade command is issued
    Then eventually the installed chart should be upgraded to the kubectl mayastor plugin's version
