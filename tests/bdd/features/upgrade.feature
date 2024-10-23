Feature: Upgrade

  Background:
    Given an installed mayastor helm chart

  Scenario: upgrade command is issued
    When a kubectl mayastor upgrade command is issued to
    Then the installed chart should be upgraded to the kubectl mayastor plugin's version
