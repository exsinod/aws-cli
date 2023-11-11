#!/bin/bash

aws sso login --profile eks-non-prod-myccv-lab-developer 
aws eks --profile eks-non-prod-myccv-lab-developer update-kubeconfig --name shared-non-prod-2
aws rds generate-db-auth-token --profile myccv-lab-non-prod-myccv-lab-developer --hostname myccv-lab-non-prod.mariadb-cxnziioaio2f.eu-west-1.rds.amazonaws.com --port 3306 --region eu-west-1 --username MyCCVLabDeveloper
#kubectl port-forward svc/mysql-lab-tunneller-mariadb -n myccv-non-prod-tunneller 3406:3306
