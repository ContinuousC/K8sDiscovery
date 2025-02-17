.PHONY: docker push-image

docker:
	-docker image rm k8s-discovery
	DOCKER_BUILDKIT=1 docker build --ssh default --target release -t k8s-discovery Discovery

push-image: docker
	docker tag k8s-discovery:latest gitea.contc/continuousc/k8s-discovery:latest
	docker push gitea.contc/continuousc/k8s-discovery:latest
